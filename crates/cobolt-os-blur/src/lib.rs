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
//! It also groups windows ([`attach_child_window`]): on macOS a window docked
//! to another is made its native child, so the window server moves the two in
//! the same screen update.
//!
//! And it rounds them ([`round_window`]): the blur follows a window's alpha, and
//! a faint pixel outside a rounded window's arc — a soft shadow's last feather —
//! would blur a rectangle behind the corner, so on macOS the window's content is
//! clipped to the arc.
//!
//! Call it on the thread that owns the windows — the main thread, where egui
//! runs its frame. It is cheap and idempotent, so a host may call it every
//! second to catch windows opened since the last call.

/// Make the window whose frame is `child` a native child of the window whose
/// frame is `parent`, so the operating system moves it WITH its parent — in
/// the same screen update, while the parent is dragged — instead of the
/// application chasing the parent a frame behind. Frames are outer rects in
/// points, `(x, y, width, height)`, top-left origin on the primary screen, as
/// egui reports them. `true` once the child is attached (already attached
/// included); `false` when either window is not found, or where the platform
/// has no such grouping (only macOS has it), so a caller keeps positioning
/// the window itself.
///
/// Call it on the thread that owns the windows, like [`set_all_windows`].
pub fn attach_child_window(parent: (f64, f64, f64, f64), child: (f64, f64, f64, f64)) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_group::attach(parent, child)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (parent, child);
        false
    }
}

/// Bring the window whose frame is `parent` — and, with it, every native child
/// window attached to it ([`attach_child_window`]) — in front of other
/// windows, without taking the keyboard from the window that has it. So a
/// click on any window of a group brings the whole group forward. `false`
/// where the window is not found or the platform has no grouping.
pub fn raise_group(parent: (f64, f64, f64, f64)) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_group::raise(parent)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = parent;
        false
    }
}

/// Round the window whose frame is `frame` to `radius` points: its content
/// layer is clipped to the arc, so every pixel outside it is fully transparent
/// whatever was painted there — a soft shadow, a glow, a faint fill — and the
/// window server's blur, which follows the window's alpha, ends at the arc too.
/// Without it the blur is a RECTANGLE as soon as anything paints one pixel with
/// the slightest alpha outside the arc, and a rounded window shows a frosted
/// block behind its corners. `frame` is the outer rect in points,
/// `(x, y, width, height)`, top-left origin on the primary screen, as
/// [`attach_child_window`] takes it. `true` once the window is rounded (rounded
/// already included); `false` when no window has that frame, or where the
/// platform has nothing to ask (only macOS does: Windows and Linux blur the
/// whole window rectangle), so a caller tries again on a later frame.
///
/// Call it on the thread that owns the windows, like [`set_all_windows`].
pub fn round_window(frame: (f64, f64, f64, f64), radius: f64) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_group::round(frame, radius)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (frame, radius);
        false
    }
}

/// The part of the screen a window may use — without the menu bar and the
/// dock (macOS) or the task bar (Windows) — as `(x, y, width, height)` in
/// points, top-left origin on the primary screen, as egui reports window
/// frames. On Windows, the screen most of `window` is on: its outer frame in
/// those points, `scale` pixels to the point. On macOS, the screen holding the
/// window that has the keyboard. `None` where the platform does not say
/// (Linux), so a caller falls back to the monitor's size.
pub fn usable_screen_area(window: (f64, f64, f64, f64), scale: f64) -> Option<(f64, f64, f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        let _ = (window, scale);
        macos_group::usable_area()
    }
    #[cfg(target_os = "windows")]
    {
        windows::work_area(window, scale)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, scale);
        None
    }
}

/// `frame`, in points at `scale` pixels to the point, as the pixel edges
/// `(left, top, right, bottom)` Windows measures screens in.
#[cfg(any(target_os = "windows", test))]
fn to_pixels((x, y, w, h): (f64, f64, f64, f64), scale: f64) -> (i32, i32, i32, i32) {
    let px = |v: f64| (v * scale).round() as i32;
    (px(x), px(y), px(x + w), px(y + h))
}

/// Pixel edges `(left, top, right, bottom)` as a frame in points at `scale`
/// pixels to the point; `None` for an empty one.
#[cfg(any(target_os = "windows", test))]
fn to_points((l, t, r, b): (i32, i32, i32, i32), scale: f64) -> Option<(f64, f64, f64, f64)> {
    (r > l && b > t && scale > 0.0)
        .then(|| (l as f64 / scale, t as f64 / scale, (r - l) as f64 / scale, (b - t) as f64 / scale))
}

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

/// macOS window groups: `-[NSWindow addChildWindow:ordered:]`, the way an
/// inspector palette stays with its document window.
#[cfg(target_os = "macos")]
mod macos_group {
    use std::ffi::{c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_stret();
    }

    // `CALayer`, which `round` talks to: loaded with the framework even where
    // nothing else of this crate reaches it (a test binary has no AppKit).
    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {}

    /// `[object frame]`: an `NSRect` comes back in registers on arm64 and
    /// through a hidden pointer (`objc_msgSend_stret`) on x86_64.
    unsafe fn frame_of(object: Id) -> Rect {
        unsafe { rect_of(object, c"frame") }
    }

    /// `[object <selector>]` for a message that answers an `NSRect`.
    unsafe fn rect_of(object: Id, selector: &std::ffi::CStr) -> Rect {
        unsafe {
            let sel = sel_registerName(selector.as_ptr());
            #[cfg(target_arch = "x86_64")]
            {
                let send: extern "C" fn(*mut Rect, Id, Sel) = std::mem::transmute(objc_msgSend_stret as *const ());
                let mut r = Rect::default();
                send(&mut r, object, sel);
                r
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                let send: extern "C" fn(Id, Sel) -> Rect = std::mem::transmute(objc_msgSend as *const ());
                send(object, sel)
            }
        }
    }

    /// Whether a Cocoa frame (bottom-left origin) is the egui frame `want`
    /// (top-left origin) on a primary screen `screen_h` points tall.
    pub(super) fn same_frame(cocoa: (f64, f64, f64, f64), want: (f64, f64, f64, f64), screen_h: f64) -> bool {
        let (x, y, w, h) = cocoa;
        let top = screen_h - (y + h);
        (x - want.0).abs() <= 2.0 && (top - want.1).abs() <= 2.0 && (w - want.2).abs() <= 2.0 && (h - want.3).abs() <= 2.0
    }

    /// The application window whose frame is `want`, if one is.
    unsafe fn window_at(want: (f64, f64, f64, f64)) -> Id {
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_usize: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let send_index: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let at = sel_registerName(c"objectAtIndex:".as_ptr());
            let count = sel_registerName(c"count".as_ptr());
            let screens = send_id(objc_getClass(c"NSScreen".as_ptr()), sel_registerName(c"screens".as_ptr()));
            if screens.is_null() || send_usize(screens, count) == 0 {
                return std::ptr::null_mut();
            }
            let screen_h = frame_of(send_index(screens, at, 0)).h;
            let app = send_id(objc_getClass(c"NSApplication".as_ptr()), sel_registerName(c"sharedApplication".as_ptr()));
            if app.is_null() {
                return std::ptr::null_mut();
            }
            let windows = send_id(app, sel_registerName(c"windows".as_ptr()));
            if windows.is_null() {
                return std::ptr::null_mut();
            }
            for i in 0..send_usize(windows, count) {
                let w = send_index(windows, at, i);
                if !w.is_null() {
                    let f = frame_of(w);
                    if same_frame((f.x, f.y, f.w, f.h), want, screen_h) {
                        return w;
                    }
                }
            }
            std::ptr::null_mut()
        }
    }

    pub(super) fn usable_area() -> Option<(f64, f64, f64, f64)> {
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_usize: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let send_index: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let screens = send_id(objc_getClass(c"NSScreen".as_ptr()), sel_registerName(c"screens".as_ptr()));
            if screens.is_null() || send_usize(screens, sel_registerName(c"count".as_ptr())) == 0 {
                return None;
            }
            let screen_h = frame_of(send_index(screens, sel_registerName(c"objectAtIndex:".as_ptr()), 0)).h;
            let main = send_id(objc_getClass(c"NSScreen".as_ptr()), sel_registerName(c"mainScreen".as_ptr()));
            if main.is_null() {
                return None;
            }
            let v = rect_of(main, c"visibleFrame");
            (v.w > 0.0 && v.h > 0.0).then(|| (v.x, screen_h - (v.y + v.h), v.w, v.h))
        }
    }

    /// Clip `layer` to a circular arc of `radius` points at its four corners:
    /// `cornerRadius` and `masksToBounds`, which make the compositor drop every
    /// pixel of the layer's content outside the arc (and so the window server
    /// drop the blur there).
    pub(super) unsafe fn round_layer(layer: Id, radius: f64) {
        unsafe {
            let send_radius: extern "C" fn(Id, Sel, f64) = std::mem::transmute(objc_msgSend as *const ());
            let send_flag: extern "C" fn(Id, Sel, i8) = std::mem::transmute(objc_msgSend as *const ());
            send_radius(layer, sel_registerName(c"setCornerRadius:".as_ptr()), radius.max(0.0));
            send_flag(layer, sel_registerName(c"setMasksToBounds:".as_ptr()), 1);
        }
    }

    /// The radius and clipping flag of `layer`, as [`round_layer`] set them.
    #[cfg(test)]
    pub(super) unsafe fn layer_rounding(layer: Id) -> (f64, bool) {
        unsafe {
            let get_radius: extern "C" fn(Id, Sel) -> f64 = std::mem::transmute(objc_msgSend as *const ());
            let get_flag: extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(objc_msgSend as *const ());
            (
                get_radius(layer, sel_registerName(c"cornerRadius".as_ptr())),
                get_flag(layer, sel_registerName(c"masksToBounds".as_ptr())) != 0,
            )
        }
    }

    /// A new, empty `CALayer`: what the content view of a window holds, without a window.
    #[cfg(test)]
    pub(super) unsafe fn new_layer() -> Id {
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let class = objc_getClass(c"CALayer".as_ptr());
            send_id(send_id(class, sel_registerName(c"alloc".as_ptr())), sel_registerName(c"init".as_ptr()))
        }
    }

    pub(super) fn round(frame: (f64, f64, f64, f64), radius: f64) -> bool {
        unsafe {
            let w = window_at(frame);
            if w.is_null() {
                return false;
            }
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_flag: extern "C" fn(Id, Sel, i8) = std::mem::transmute(objc_msgSend as *const ());
            let view = send_id(w, sel_registerName(c"contentView".as_ptr()));
            if view.is_null() {
                return false;
            }
            // The renderer makes this view's layer a CAMetalLayer; asking for a
            // layer here changes nothing for a view that already has one.
            send_flag(view, sel_registerName(c"setWantsLayer:".as_ptr()), 1);
            let layer = send_id(view, sel_registerName(c"layer".as_ptr()));
            if layer.is_null() {
                return false;
            }
            round_layer(layer, radius);
            true
        }
    }

    pub(super) fn raise(parent: (f64, f64, f64, f64)) -> bool {
        unsafe {
            let w = window_at(parent);
            if w.is_null() {
                return false;
            }
            // orderFront: brings the window and its child windows forward and
            // leaves the key window as it is.
            let send_obj: extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
            send_obj(w, sel_registerName(c"orderFront:".as_ptr()), std::ptr::null_mut());
            true
        }
    }

    pub(super) fn attach(parent: (f64, f64, f64, f64), child: (f64, f64, f64, f64)) -> bool {
        unsafe {
            let send_id: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_usize: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let send_index: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let send_add: extern "C" fn(Id, Sel, Id, isize) = std::mem::transmute(objc_msgSend as *const ());

            let at = sel_registerName(c"objectAtIndex:".as_ptr());
            let count = sel_registerName(c"count".as_ptr());
            let screens = send_id(objc_getClass(c"NSScreen".as_ptr()), sel_registerName(c"screens".as_ptr()));
            if screens.is_null() || send_usize(screens, count) == 0 {
                return false;
            }
            let screen_h = frame_of(send_index(screens, at, 0)).h;
            let app = send_id(objc_getClass(c"NSApplication".as_ptr()), sel_registerName(c"sharedApplication".as_ptr()));
            if app.is_null() {
                return false;
            }
            let windows = send_id(app, sel_registerName(c"windows".as_ptr()));
            if windows.is_null() {
                return false;
            }
            let (mut p, mut c): (Id, Id) = (std::ptr::null_mut(), std::ptr::null_mut());
            for i in 0..send_usize(windows, count) {
                let w = send_index(windows, at, i);
                if w.is_null() {
                    continue;
                }
                let f = frame_of(w);
                let f = (f.x, f.y, f.w, f.h);
                if p.is_null() && same_frame(f, parent, screen_h) {
                    p = w;
                } else if c.is_null() && same_frame(f, child, screen_h) {
                    c = w;
                }
            }
            if p.is_null() || c.is_null() || p == c {
                return false;
            }
            if send_id(c, sel_registerName(c"parentWindow".as_ptr())) == p {
                return true;
            }
            // NSWindowAbove = 1: the child stays above its parent.
            send_add(p, sel_registerName(c"addChildWindow:ordered:".as_ptr()), c, 1);
            true
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

    /// `RECT`.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    /// `MONITORINFO`.
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    /// `MONITOR_DEFAULTTONEAREST`.
    const DEFAULT_TO_NEAREST: u32 = 2;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumThreadWindows(
            thread: u32,
            callback: unsafe extern "system" fn(Hwnd, isize) -> Bool,
            param: isize,
        ) -> Bool;
        fn MonitorFromRect(rect: *const Rect, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> Bool;
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

    /// The work area (the monitor less the task bar) of the monitor most of
    /// `window` is on — the one Windows itself says the window is on. Screens
    /// are measured in pixels across the whole desktop, the primary monitor's
    /// top-left at (0, 0), so a monitor left of or above it starts below zero.
    pub(super) fn work_area(window: (f64, f64, f64, f64), scale: f64) -> Option<(f64, f64, f64, f64)> {
        let (left, top, right, bottom) = super::to_pixels(window, scale);
        let rect = Rect { left, top, right, bottom };
        unsafe {
            let monitor = MonitorFromRect(&rect, DEFAULT_TO_NEAREST);
            if monitor.is_null() {
                return None;
            }
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return None;
            }
            let w = info.work;
            super::to_points((w.left, w.top, w.right, w.bottom), scale)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn support_matches_the_platform() {
        assert_eq!(super::supported(), cfg!(any(target_os = "macos", target_os = "windows")));
    }

    /// Cocoa counts y up from the primary screen's bottom; egui counts it down
    /// from the top. A 1000×600 window whose top-left is at (100, 200) on a
    /// 1117-point screen has its Cocoa origin at y = 1117 − 200 − 600 = 317.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_cocoa_frame_is_matched_against_eguis_top_left_one() {
        use super::macos_group::same_frame;
        let egui = (100.0, 200.0, 1000.0, 600.0);
        assert!(same_frame((100.0, 317.0, 1000.0, 600.0), egui, 1117.0));
        assert!(same_frame((101.5, 318.0, 1000.0, 600.0), egui, 1117.0), "within two points");
        assert!(!same_frame((100.0, 200.0, 1000.0, 600.0), egui, 1117.0), "not with y left uncounted");
        assert!(!same_frame((100.0, 317.0, 900.0, 600.0), egui, 1117.0), "another size is another window");
    }

    /// A window on a laptop at 150 % that sits right of a 2560-pixel monitor
    /// is found by its pixels, and the laptop's work area comes back in the
    /// same points as the window: starting where the laptop starts, not at
    /// the primary monitor's corner.
    #[test]
    fn a_frame_goes_to_pixels_and_a_work_area_back_to_points() {
        let window = (2600.0 / 1.5, 40.0 / 1.5, 384.0 / 1.5, 312.0 / 1.5);
        assert_eq!(super::to_pixels(window, 1.5), (2600, 40, 2984, 352));
        let laptop = super::to_points((2560, 0, 4480, 1032), 1.5).unwrap();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(laptop.0, 2560.0 / 1.5) && near(laptop.1, 0.0), "{laptop:?}");
        assert!(near(laptop.2, 1280.0) && near(laptop.3, 688.0), "{laptop:?}");
        assert_eq!(super::to_points((-1920, 0, 0, 1040), 1.0), Some((-1920.0, 0.0, 1920.0, 1040.0)), "left of the primary");
        assert_eq!(super::to_points((10, 10, 10, 20), 1.0), None, "an empty area is no area");
    }

    /// Off the main thread of an app with no windows, nothing is found and
    /// nothing is attached — the caller keeps placing the window itself.
    #[test]
    fn nothing_is_attached_where_no_window_matches() {
        assert!(!super::attach_child_window((0.0, 0.0, 10.0, 10.0), (20.0, 0.0, 10.0, 10.0)));
    }

    /// Nothing is rounded where no window has the frame: the caller asks again
    /// on a later frame, once the window exists.
    #[test]
    fn nothing_is_rounded_where_no_window_matches() {
        assert!(!super::round_window((0.0, 0.0, 10.0, 10.0), 35.0));
    }

    /// The layer a window shows is clipped to the arc: its corner radius is the
    /// form's, and it masks to its bounds — the two properties that make the
    /// compositor, and so the window server's blur, stop at the arc. (Seen once, on
    /// the screen, in a stand-alone Metal-backed window with a rectangle painted
    /// over all of it: square blur without the two properties, arc-shaped with
    /// them. A test cannot open a window, so this one checks the properties.)
    #[cfg(target_os = "macos")]
    #[test]
    fn a_layer_is_clipped_to_the_arc_it_is_given() {
        use super::macos_group::{layer_rounding, new_layer, round_layer};
        unsafe {
            let layer = new_layer();
            assert!(!layer.is_null(), "CoreAnimation answers a layer");
            assert_eq!(layer_rounding(layer), (0.0, false), "a new layer is square and lets paint overflow");
            round_layer(layer, 35.0);
            assert_eq!(layer_rounding(layer), (35.0, true));
            round_layer(layer, 29.5);
            assert_eq!(layer_rounding(layer), (29.5, true), "a window resized to another arc follows it");
            round_layer(layer, -3.0);
            assert_eq!(layer_rounding(layer), (0.0, true), "a negative radius is no radius");
        }
    }
}
