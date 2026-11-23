// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The real machine for [`gather`], and the entry point `PrcFacts` that the `.msi` names (task T11).
//!
//! `PrcFacts` is an **immediate** action, so it runs as the person at the keyboard (or, in a silent install, as
//! whoever runs `msiexec`), which is the account whose Rust and build folder the page is about. It reads what the
//! installer knows from properties, gathers the rest from the machine, decides ([`prepare::decide`]) and sets the
//! properties the page reads. It never fails the install: a panic or a missing property is written to the log and
//! the page simply is not offered.

use super::{gather, prepare_property, properties, volume_name, Inputs, Probe};
use crate::disk::Volume;
use crate::figures::FIGURES;
use crate::prepare::{decide, Mode};
use crate::supervisor::{guarded, wide, Session};
use std::io::Read as _;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{MsiSetPropertyW, MSIHANDLE};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// The longest any one question of the page may take: `rustup` and `vswhere` answer at once, so a program that
/// does not is stuck, and the installer's own page must not wait for it.
const ASK_LIMIT: Duration = Duration::from_secs(20);

/// `UILevel` 5 is the full wizard; every other level (`/qn`, `/qb`, `/passive`) cannot show the page.
const UI_FULL: &str = "5";

/// This computer, as the installing user.
pub struct Here;

impl Probe for Here {
    fn output(&mut self, program: &str, args: &[&str]) -> Option<String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .ok()?;
        let mut stdout = child.stdout.take()?;
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let began = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if began.elapsed() < ASK_LIMIT => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        let bytes = reader.join().unwrap_or_default();
        status
            .filter(|s| s.success())
            .map(|_| String::from_utf8_lossy(&bytes).into_owned())
    }

    fn files_in(&mut self, dir: &str) -> Option<Vec<String>> {
        let entries = std::fs::read_dir(dir).ok()?;
        Some(
            entries
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .collect(),
        )
    }

    fn free_on(&mut self, path: &str) -> Option<Volume> {
        // The build folder may not exist yet: its volume is the one its nearest existing parent is on.
        let mut existing = Path::new(path);
        while !existing.exists() {
            existing = existing.parent()?;
        }
        let wide_path = wide(existing.to_str()?);
        let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
        // SAFETY: `wide_path` is NUL-terminated and the three counters are valid for the call.
        let ok = unsafe {
            GetDiskFreeSpaceExW(wide_path.as_ptr(), &mut available, &mut total, &mut free)
        };
        (ok != 0).then(|| Volume {
            name: volume_name(path),
            // What is free *for this user* (quotas count), which is what the compile can use.
            free_bytes: available,
        })
    }

    fn var(&mut self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
}

/// Set (or, for an empty value, remove) a property of the install.
fn set_property(session: &Session, name: &str, value: &str) {
    let (name, value) = (wide(name), wide(value));
    // SAFETY: both strings are NUL-terminated; an empty value removes the property.
    unsafe {
        MsiSetPropertyW(session.0, name.as_ptr(), value.as_ptr());
    }
}

/// What the installer knows, from the session.
fn inputs(session: &Session) -> Option<Inputs> {
    let text = |name: &str| session.property(name).filter(|v| !v.trim().is_empty());
    let mode = if session.property("UILevel").as_deref() == Some(UI_FULL) {
        Mode::Interactive
    } else {
        Mode::Silent {
            opt_in: session.property("PREPARE_PLATFORM").as_deref() == Some("1"),
        }
    };
    Some(Inputs {
        version: text("ProductVersion")?,
        // The release workflow sets it from the manifest, as it does the Rust it builds with.
        pinned_rust: text("PRC_PINNED_RUST")?,
        // `<UpgradeVersion Property="PRC_PREVIOUS_VERSION">` finds the older product being replaced.
        previous_version: text("PRC_PREVIOUS_VERSION"),
        mode,
    })
}

/// The entry point the `.msi` names (`DllEntry="PrcFacts"`, an immediate action before the preparation page):
/// gather the facts, decide, and set `PRC_TEXT`, `PRC_NEED`, `PRC_DISK_OK`, `PRC_ALREADY`, `PRC_CANCEL_TEXT` and,
/// for a silent install, `PRC_PREPARE`.
#[no_mangle]
pub extern "system" fn PrcFacts(handle: MSIHANDLE) -> u32 {
    guarded(handle, |session| {
        let Some(inputs) = inputs(session) else {
            session.info(
                "PRC: ProductVersion or PRC_PINNED_RUST is not set; no preparation is offered",
            );
            return ERROR_SUCCESS;
        };
        let facts = gather(&mut Here, &inputs);
        let plan = decide(&facts, &FIGURES);
        session.info(&format!(
            "PRC: facts {facts:?}; offer {:?}; steps {:?}",
            plan.offer, plan.steps
        ));
        for (name, value) in properties(&plan) {
            set_property(session, name, &value);
        }
        if let Some((name, value)) = prepare_property(&plan, inputs.mode) {
            set_property(session, name, &value);
        }
        ERROR_SUCCESS
    })
}
