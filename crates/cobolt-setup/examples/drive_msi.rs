// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Installs an `.msi` and answers Cancel like a person would (spec 112, task T1).
//!
//! A hosted runner has nobody at the keyboard, so the only way to exercise a
//! custom action's Cancel handling is to be the user interface: this registers
//! an external UI handler, prints every message the installer sends it, and
//! returns `IDCANCEL` once a chosen amount of time has passed since a chosen
//! text was first seen.
//!
//! ```text
//! drive_msi <package.msi> <log file> <cancel after seconds | never> <text to start counting at | -> [NAME=value ...]
//! ```
//!
//! Exit code: the installer's own (0 installed, 1602 cancelled by the user,
//! 1603 fatal error, ...).

#[cfg(not(windows))]
fn main() {
    eprintln!("drive_msi installs a Windows Installer package and runs on Windows only");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    imp::main();
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use windows_sys::core::PCWSTR;
    use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
        MsiEnableLogW, MsiInstallProductW, MsiSetExternalUIW, MsiSetInternalUI,
        INSTALLLOGMODE_ACTIONDATA, INSTALLLOGMODE_ACTIONSTART, INSTALLLOGMODE_COMMONDATA,
        INSTALLLOGMODE_ERROR, INSTALLLOGMODE_FATALEXIT, INSTALLLOGMODE_INFO,
        INSTALLLOGMODE_PROGRESS, INSTALLLOGMODE_USER, INSTALLLOGMODE_WARNING, INSTALLUILEVEL_NONE,
    };

    struct Plan {
        /// Seconds after the marker was first seen at which Cancel is answered; `None` never cancels.
        cancel_after: Option<f64>,
        /// The text that starts the clock; empty starts it at the first message.
        marker: String,
    }

    static PLAN: OnceLock<Plan> = OnceLock::new();
    static BEGUN: OnceLock<Instant> = OnceLock::new();
    static MARKED: Mutex<Option<Instant>> = Mutex::new(None);
    static ACTION_DATA: AtomicU32 = AtomicU32::new(0);
    static PROGRESS: AtomicU32 = AtomicU32::new(0);
    static CANCELS_SENT: AtomicU32 = AtomicU32::new(0);

    const INFO: u32 = 0x0400_0000;
    const ACTIONSTART: u32 = 0x0800_0000;
    const ACTIONDATA: u32 = 0x0900_0000;
    const PROGRESS_KIND: u32 = 0x0A00_0000;

    fn text_of(message: PCWSTR) -> String {
        if message.is_null() {
            return String::new();
        }
        // SAFETY: the installer passes a NUL-terminated string valid for the call.
        unsafe {
            let mut len = 0usize;
            while *message.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(message, len))
        }
    }

    unsafe extern "system" fn handler(_context: *mut c_void, kind: u32, message: PCWSTR) -> i32 {
        let begun = *BEGUN.get_or_init(Instant::now);
        let text = text_of(message);
        let class = kind & 0xFF00_0000;
        match class {
            ACTIONDATA => ACTION_DATA.fetch_add(1, Ordering::Relaxed),
            PROGRESS_KIND => PROGRESS.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
        // Progress polls arrive twice a second per running step; printing each would bury the log.
        if class != PROGRESS_KIND {
            println!(
                "[+{:8.1}s] {:#010x} {}",
                begun.elapsed().as_secs_f64(),
                class,
                text.replace('\n', " | ")
            );
        }

        let Some(plan) = PLAN.get() else { return 1 };
        let mut marked = MARKED.lock().unwrap_or_else(|p| p.into_inner());
        if marked.is_none() && (plan.marker.is_empty() || text.contains(&plan.marker)) {
            *marked = Some(Instant::now());
            println!(
                "[+{:8.1}s] -- the clock starts (saw '{}')",
                begun.elapsed().as_secs_f64(),
                plan.marker
            );
        }
        if let (Some(after), Some(since)) = (plan.cancel_after, *marked) {
            let answerable = class == ACTIONDATA || class == PROGRESS_KIND || class == ACTIONSTART;
            if answerable && since.elapsed().as_secs_f64() >= after {
                if CANCELS_SENT.fetch_add(1, Ordering::Relaxed) == 0 {
                    println!(
                        "[+{:8.1}s] -- CANCEL: answering IDCANCEL",
                        begun.elapsed().as_secs_f64()
                    );
                }
                return 2; // IDCANCEL
            }
        }
        // The setup's own lines ("PRC: ...") are INFO messages, meant for the log. Answering "handled" for them is
        // not what `msiexec` does, and the first runs on a hosted machine found none of them in the log, so they
        // get the installer's own handling.
        if class == INFO {
            0
        } else {
            1 // handled
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.len() < 4 {
            eprintln!("usage: drive_msi <package.msi> <log file> <cancel after seconds | never> <marker | -> [NAME=value ...]");
            std::process::exit(64);
        }
        let cancel_after = match args[2].as_str() {
            "never" => None,
            secs => Some(secs.parse::<f64>().unwrap_or_else(|_| {
                eprintln!("'{secs}' is not a number of seconds");
                std::process::exit(64)
            })),
        };
        let marker = if args[3] == "-" {
            String::new()
        } else {
            args[3].clone()
        };
        let _ = PLAN.set(Plan {
            cancel_after,
            marker,
        });
        let _ = BEGUN.set(Instant::now());
        let properties = args[4..].join(" ");

        let package = wide(&args[0]);
        let log = wide(&args[1]);
        let command = wide(&properties);
        let filter = (INSTALLLOGMODE_FATALEXIT
            | INSTALLLOGMODE_ERROR
            | INSTALLLOGMODE_WARNING
            | INSTALLLOGMODE_USER
            | INSTALLLOGMODE_INFO
            | INSTALLLOGMODE_ACTIONSTART
            | INSTALLLOGMODE_ACTIONDATA
            | INSTALLLOGMODE_PROGRESS
            | INSTALLLOGMODE_COMMONDATA) as u32;

        println!("drive_msi: {} {}", args[0], properties);
        // SAFETY: every string is NUL-terminated and outlives the calls; the handler has the documented signature.
        let code = unsafe {
            MsiSetInternalUI(INSTALLUILEVEL_NONE, std::ptr::null_mut());
            MsiSetExternalUIW(Some(handler), filter, std::ptr::null());
            // Everything, verbose, as `/l*v` writes it.
            MsiEnableLogW(0x3FFF, log.as_ptr(), 0);
            MsiInstallProductW(package.as_ptr(), command.as_ptr())
        };
        println!(
            "drive_msi: MsiInstallProductW returned {code} after {:.1}s; {} action-data and {} progress messages reached the handler; {} Cancel answers sent",
            BEGUN.get().map_or(0.0, |b| b.elapsed().as_secs_f64()),
            ACTION_DATA.load(Ordering::Relaxed),
            PROGRESS.load(Ordering::Relaxed),
            CANCELS_SENT.load(Ordering::Relaxed),
        );
        std::process::exit(code as i32);
    }
}
