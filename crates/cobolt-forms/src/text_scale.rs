// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The system text factor** (spec 056 R68): the operating system's own
//! "make text bigger" setting, which a responsive form's font factor is
//! multiplied by.
//!
//! | Platform | Where it is read | When |
//! |---|---|---|
//! | Windows | `HKCU\Software\Microsoft\Accessibility\TextScaleFactor` (Settings → Accessibility → Text size), a percentage 100–225 | once, at start-up |
//! | Linux (GNOME) | `gsettings get org.gnome.desktop.interface text-scaling-factor` | once, at start-up |
//! | macOS | nowhere — macOS has no system-wide text size an application reads; its display scaling is already in egui's pixels-per-point | always 1.0 |
//!
//! Anything missing or unreadable is 1.0: the factor can only ever make a
//! form's text follow a setting the user chose, never break it. The Windows
//! read is hand-written FFI to `advapi32.dll`, like the IDE's credential
//! store — no crate is added for one registry value.

use std::sync::OnceLock;

/// The factor, read once per process.
pub fn system_text_factor() -> f32 {
    static FACTOR: OnceLock<f32> = OnceLock::new();
    *FACTOR.get_or_init(|| sane(platform::read()))
}

/// A factor outside what any platform offers is a misread: 1.0.
fn sane(f: Option<f32>) -> f32 {
    const LOWEST: f32 = 0.5;
    const HIGHEST: f32 = 3.0;
    f.filter(|v| v.is_finite() && (LOWEST..=HIGHEST).contains(v)).unwrap_or(1.0)
}

/// Windows stores a percentage.
#[cfg_attr(not(windows), allow(dead_code))]
fn from_percent(p: u32) -> f32 {
    p as f32 / 100.0
}

/// `gsettings` prints a GVariant: `1.25`, or `double 1.25` for some types.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn from_gsettings(out: &str) -> Option<f32> {
    out.split_whitespace().last()?.parse().ok()
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    type Hkey = isize;
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: Hkey,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn read() -> Option<f32> {
        let key = wide("Software\\Microsoft\\Accessibility");
        let name = wide("TextScaleFactor");
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the
        // call; `data` points at a u32 and `size` says so.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        (status == 0).then(|| super::from_percent(value))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    pub fn read() -> Option<f32> {
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        super::from_gsettings(&String::from_utf8_lossy(&out.stdout))
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    pub fn read() -> Option<f32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_convert_and_nonsense_is_one() {
        assert_eq!(from_percent(125), 1.25);
        assert_eq!(from_gsettings("1.25\n"), Some(1.25));
        assert_eq!(from_gsettings("double 1.5"), Some(1.5));
        assert_eq!(from_gsettings("No such schema"), None);
        assert_eq!(sane(Some(1.25)), 1.25);
        assert_eq!(sane(Some(0.0)), 1.0);
        assert_eq!(sane(Some(f32::NAN)), 1.0);
        assert_eq!(sane(None), 1.0);
        println!("system text factor on this machine: {}", system_text_factor());
    }
}
