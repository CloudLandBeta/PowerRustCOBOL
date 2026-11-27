// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The title bar of a see-through window, looked at in a real (hidden) window.
//!
//! `cobolt_os_blur::fill_title_bars` gives a transparent window's title bar the system's own colour. Its
//! first version added the fill to the window's FRAME VIEW and AppKit answered by dissolving the title bar
//! and laying the title out again, at the left edge under the buttons (2.1.73, seen on a screen). The
//! checks here are what would have shown it: the title keeps its place, the bar is the colour an ordinary
//! window's bar is (light and dark), the content stays see-through, a second call changes nothing, and a
//! resized window keeps all of it.
//!
//! AppKit makes windows on the main thread only, so this test is its own `main` (`harness = false`). The
//! windows are never shown (fully transparent, ignoring the mouse). Without a window-server session
//! (a build machine over ssh) there is nothing to look at and the test says so and passes.

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("title_bar: macOS only, nothing to check on this system");
}

#[cfg(target_os = "macos")]
fn main() {
    mac::run();
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void, CStr};
    use std::time::Instant;

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    }
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        /// Null when this process has no window-server session.
        fn CGSessionCopyCurrentDictionary() -> Id;
    }

    fn sel(name: &CStr) -> Sel {
        unsafe { sel_registerName(name.as_ptr()) }
    }
    fn class(name: &CStr) -> Id {
        unsafe { objc_getClass(name.as_ptr()) }
    }
    fn get_id(o: Id, s: &CStr) -> Id {
        unsafe {
            let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s))
        }
    }
    fn get_usize(o: Id, s: &CStr) -> usize {
        unsafe {
            let f: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s))
        }
    }
    fn get_rect(o: Id, s: &CStr) -> Rect {
        unsafe {
            let f: extern "C" fn(Id, Sel) -> Rect = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s))
        }
    }
    fn nth(array: Id, i: usize) -> Id {
        unsafe {
            let f: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            f(array, sel(c"objectAtIndex:"), i)
        }
    }
    fn set_bool(o: Id, s: &CStr, v: bool) {
        unsafe {
            let f: extern "C" fn(Id, Sel, i8) = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s), v as i8)
        }
    }
    fn set_id(o: Id, s: &CStr, v: Id) {
        unsafe {
            let f: extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s), v)
        }
    }
    fn set_f64(o: Id, s: &CStr, v: f64) {
        unsafe {
            let f: extern "C" fn(Id, Sel, f64) = std::mem::transmute(objc_msgSend as *const ());
            f(o, sel(s), v)
        }
    }
    fn ns_string(s: &CStr) -> Id {
        unsafe {
            let f: extern "C" fn(Id, Sel, *const c_char) -> Id = std::mem::transmute(objc_msgSend as *const ());
            f(class(c"NSString"), sel(c"stringWithUTF8String:"), s.as_ptr())
        }
    }
    fn rust_string(s: Id) -> String {
        if s.is_null() {
            return String::new();
        }
        unsafe {
            let f: extern "C" fn(Id, Sel) -> *const c_char = std::mem::transmute(objc_msgSend as *const ());
            CStr::from_ptr(f(s, sel(c"UTF8String"))).to_string_lossy().into_owned()
        }
    }
    fn class_name(view: Id) -> String {
        rust_string(get_id(view, c"className"))
    }

    /// Let the window server and AppKit do what they do after a window is made or changed.
    fn spin(seconds: f64) {
        unsafe {
            let date: extern "C" fn(Id, Sel, f64) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let until = date(class(c"NSDate"), sel(c"dateWithTimeIntervalSinceNow:"), seconds);
            let run: extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
            run(get_id(class(c"NSRunLoop"), c"currentRunLoop"), sel(c"runUntilDate:"), until);
        }
    }

    /// A titled window like the one winit makes (`transparent` = what `with_transparent(true)` does), never
    /// shown: fully transparent and deaf to the mouse.
    fn make_window(title: &CStr, transparent: bool, dark: bool) -> Id {
        unsafe {
            let init: extern "C" fn(Id, Sel, Rect, usize, usize, i8) -> Id = std::mem::transmute(objc_msgSend as *const ());
            // titled | closable | miniaturizable | resizable; buffered; not deferred
            let window = init(
                get_id(class(c"NSWindow"), c"alloc"),
                sel(c"initWithContentRect:styleMask:backing:defer:"),
                Rect { x: 100.0, y: 100.0, w: 1280.0, h: 800.0 },
                1 | 2 | 4 | 8,
                2,
                0,
            );
            set_bool(window, c"setReleasedWhenClosed:", false);
            set_id(window, c"setTitle:", ns_string(title));
            if transparent {
                set_bool(window, c"setOpaque:", false);
                set_id(window, c"setBackgroundColor:", get_id(class(c"NSColor"), c"clearColor"));
            }
            if dark {
                let named: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
                let appearance = named(class(c"NSAppearance"), sel(c"appearanceNamed:"), ns_string(c"NSAppearanceNameDarkAqua"));
                set_id(window, c"setAppearance:", appearance);
            }
            set_f64(window, c"setAlphaValue:", 0.0);
            set_bool(window, c"setIgnoresMouseEvents:", true);
            set_id(window, c"orderFront:", std::ptr::null_mut());
            spin(0.5);
            window
        }
    }

    fn close(window: Id) {
        unsafe {
            let f: extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const ());
            f(window, sel(c"close"));
        }
    }

    fn frame_view(window: Id) -> Id {
        get_id(get_id(window, c"contentView"), c"superview")
    }

    /// The frame of the text field that shows the title, wherever AppKit keeps it.
    fn title_field(view: Id, title: &str) -> Option<Rect> {
        unsafe {
            let responds: extern "C" fn(Id, Sel, Sel) -> i8 = std::mem::transmute(objc_msgSend as *const ());
            if class_name(view) == "NSTextField" && responds(view, sel(c"respondsToSelector:"), sel(c"stringValue")) != 0 && rust_string(get_id(view, c"stringValue")) == title {
                return Some(get_rect(view, c"frame"));
            }
        }
        let subviews = get_id(view, c"subviews");
        if subviews.is_null() {
            return None;
        }
        (0..get_usize(subviews, c"count")).find_map(|i| title_field(nth(subviews, i), title))
    }

    /// Whether the title bar's container is still a subview of the frame view.
    fn has_title_bar_container(window: Id) -> bool {
        let subviews = get_id(frame_view(window), c"subviews");
        (0..get_usize(subviews, c"count")).any(|i| class_name(nth(subviews, i)) == "NSTitlebarContainerView")
    }

    /// `(title bar pixel, content pixel)` as RGBA, read from what the frame view draws: a point in the middle
    /// of the bar clear of the buttons and the title, and one in the middle of the content.
    fn pixels(window: Id) -> ([u8; 4], [u8; 4]) {
        unsafe {
            let view = frame_view(window);
            let bounds = get_rect(view, c"bounds");
            let make: extern "C" fn(Id, Sel, Rect) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let rep = make(view, sel(c"bitmapImageRepForCachingDisplayInRect:"), bounds);
            assert!(!rep.is_null(), "AppKit gives a bitmap to draw the window in");
            let cache: extern "C" fn(Id, Sel, Rect, Id) = std::mem::transmute(objc_msgSend as *const ());
            cache(view, sel(c"cacheDisplayInRect:toBitmapImageRep:"), bounds, rep);
            let data: extern "C" fn(Id, Sel) -> *const u8 = std::mem::transmute(objc_msgSend as *const ());
            let bytes = data(rep, sel(c"bitmapData"));
            let (per_row, per_pixel, wide) = (get_usize(rep, c"bytesPerRow"), get_usize(rep, c"samplesPerPixel"), get_usize(rep, c"pixelsWide"));
            assert_eq!(per_pixel, 4, "the bitmap is RGBA");
            let scale = wide as f64 / bounds.w;
            let at = |x: f64, from_top: f64| -> [u8; 4] {
                let offset = (from_top * scale) as usize * per_row + (x * scale) as usize * per_pixel;
                [*bytes.add(offset), *bytes.add(offset + 1), *bytes.add(offset + 2), *bytes.add(offset + 3)]
            };
            // 16 points from the top is the middle of a 32-point bar; 400 from the top is inside the content.
            (at(900.0, 16.0), at(900.0, 400.0))
        }
    }

    fn resize(window: Id, w: f64, h: f64) {
        unsafe {
            let f: extern "C" fn(Id, Sel, Rect, i8) = std::mem::transmute(objc_msgSend as *const ());
            f(window, sel(c"setFrame:display:"), Rect { x: 100.0, y: 100.0, w, h }, 1);
        }
        spin(0.5);
    }

    pub fn run() {
        let started = Instant::now();
        unsafe {
            let session = CGSessionCopyCurrentDictionary();
            if session.is_null() {
                println!("title_bar: no window-server session in this process, nothing to look at (passes)");
                return;
            }
            let app = get_id(class(c"NSApplication"), c"sharedApplication");
            let policy: extern "C" fn(Id, Sel, isize) -> i8 = std::mem::transmute(objc_msgSend as *const ());
            policy(app, sel(c"setActivationPolicy:"), 1); // accessory: no Dock icon
            let finish: extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const ());
            finish(app, sel(c"finishLaunching"));
        }

        let title = c"PowerRustCOBOL AI Desktop RAD 2.1.74";
        let title_str = title.to_str().unwrap();
        let mut checks = 0;
        let mut check = |ok: bool, what: String| {
            checks += 1;
            assert!(ok, "title_bar: {what}");
        };
        let mut rows = Vec::new();

        for (dark, label) in [(false, "light"), (true, "dark")] {
            // The bar an ordinary window has, to compare with: opaque, nothing added.
            let reference = make_window(title, false, dark);
            let (bar_wanted, _) = pixels(reference);
            check(bar_wanted[3] == 255, format!("{label}: an ordinary window's bar is opaque, got {bar_wanted:?}"));

            let window = make_window(title, true, dark);
            let place_before = title_field(frame_view(window), title_str).expect("the title is shown");
            let (bar_before, _) = pixels(window);
            check(bar_before[3] == 0, format!("{label}: a see-through window's bar is see-through before the fill (the bug it fixes), got {bar_before:?}"));

            let given = cobolt_os_blur::fill_title_bars();
            spin(0.5);
            check(given == 1, format!("{label}: one window was given a fill, {given}"));
            check(has_title_bar_container(window), format!("{label}: the system's title bar is still there"));
            let place_after = title_field(frame_view(window), title_str).expect("the title is still shown");
            check(place_after == place_before, format!("{label}: the title keeps its place, {place_before:?} became {place_after:?}"));
            let (bar_after, content_after) = pixels(window);
            check(bar_after == bar_wanted, format!("{label}: the bar is the system's colour {bar_wanted:?}, got {bar_after:?}"));
            check(content_after[3] == 0, format!("{label}: the content stays see-through, got {content_after:?}"));
            check(cobolt_os_blur::fill_title_bars() == 0, format!("{label}: a second call gives no second fill"));

            resize(window, 1100.0, 700.0);
            let place_resized = title_field(frame_view(window), title_str).expect("the title is shown after a resize");
            check(
                place_resized.x == place_before.x && place_resized.w == place_before.w,
                format!("{label}: the title keeps its place when the window is resized, {place_before:?} became {place_resized:?}"),
            );
            let (bar_resized, content_resized) = pixels(window);
            check(bar_resized == bar_wanted, format!("{label}: the bar keeps its colour when the window is resized, got {bar_resized:?}"));
            check(content_resized[3] == 0, format!("{label}: the content stays see-through when the window is resized"));

            rows.push(format!(
                "  {label:<5} title at x={:.0} width={:.0} (before and after the fill, and resized)   bar before {:?}  after {:?}  ordinary window {:?}",
                place_before.x, place_before.w, bar_before, bar_after, bar_wanted
            ));
            close(window);
            close(reference);
        }

        println!("title_bar: window \"{title_str}\", see-through and titled, 1280x800 then 1100x700, never shown");
        for row in &rows {
            println!("{row}");
        }
        println!("title_bar: {checks} checks passed, 0 failed, {} ms", started.elapsed().as_millis());
    }
}
