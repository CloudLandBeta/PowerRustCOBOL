// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Android devices and emulators** a Mobile application runs on (WASM-012 T16, T20),
//! reached with the SDK's `adb`: the ones connected, the processor of one (the package is built
//! for it), installing a package, starting the application, and its log.
//!
//! What `adb` prints is read by small parsers ([`parse_devices`], [`is_application_log`]) so the
//! answers are tested from recorded output; the commands themselves need a device.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::mobile_build::{android_sdk, AndroidAbi};

/// A device or emulator `adb` sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// `emulator-5554`, or a phone's serial number.
    pub serial: String,
    /// `device` when it can be used; `offline`, `unauthorized` (the phone has not accepted this
    /// computer yet)…
    pub state: String,
    /// The model `adb devices -l` names (`sdk_gphone64_arm64`, `Pixel_8`), or empty.
    pub model: String,
}

impl Device {
    /// Ready to install on.
    pub fn ready(&self) -> bool {
        self.state == "device"
    }

    /// How it is named to the developer: the model and the serial.
    pub fn label(&self) -> String {
        if self.model.is_empty() {
            self.serial.clone()
        } else {
            format!("{} ({})", self.model.replace('_', " "), self.serial)
        }
    }
}

/// The devices in `adb devices -l`'s answer.
pub fn parse_devices(text: &str) -> Vec<Device> {
    text.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?.to_owned();
            let state = parts.next()?.to_owned();
            let model = parts.find_map(|p| p.strip_prefix("model:")).unwrap_or("").to_owned();
            Some(Device { serial, state, model })
        })
        .collect()
}

/// The log tags of a Mobile application: the page's console (Tauri's web view), the shell's
/// own output, and a crash.
pub const LOG_TAGS: &[&str] = &["Tauri/Console:V", "RustStdoutStderr:V", "AndroidRuntime:E", "*:S"];

/// Is `line` (of `adb logcat -v brief`) one of the application's? The page's console and a crash
/// are; of the process's own output only the shell's lines are (the web view writes its internal
/// `[INFO:…]` lines there too).
pub fn is_application_log(line: &str) -> bool {
    line.contains("/Tauri/Console") || line.starts_with("E/AndroidRuntime") || (line.contains("/RustStdoutStderr") && line.contains("PowerRustCOBOL"))
}

/// The SDK's `adb`.
#[derive(Debug, Clone)]
pub struct Adb {
    pub path: PathBuf,
}

impl Adb {
    /// `adb` of the Android SDK [`android_sdk`] finds.
    pub fn find() -> Option<Self> {
        let sdk = android_sdk()?;
        Self::in_sdk(&sdk)
    }

    /// `adb` of the SDK at `sdk`.
    pub fn in_sdk(sdk: &Path) -> Option<Self> {
        let path = sdk.join("platform-tools").join(if cfg!(windows) { "adb.exe" } else { "adb" });
        path.is_file().then_some(Self { path })
    }

    fn command(&self, serial: Option<&str>) -> Command {
        let mut c = Command::new(&self.path);
        if let Some(s) = serial {
            c.args(["-s", s]);
        }
        c.stdin(Stdio::null());
        c
    }

    fn run(&self, serial: Option<&str>, args: &[&str]) -> Result<String, String> {
        let out = self
            .command(serial)
            .args(args)
            .output()
            .map_err(|e| format!("adb could not be started ({e})"))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
        if out.status.success() {
            Ok(text)
        } else {
            Err(format!("adb {} failed: {}", args.join(" "), text.trim()))
        }
    }

    /// The devices and emulators connected now.
    pub fn devices(&self) -> Vec<Device> {
        self.run(None, &["devices", "-l"]).map(|t| parse_devices(&t)).unwrap_or_default()
    }

    /// The processor of `serial` (`ro.product.cpu.abi`).
    pub fn abi(&self, serial: &str) -> Option<AndroidAbi> {
        self.run(Some(serial), &["shell", "getprop", "ro.product.cpu.abi"]).ok().and_then(|t| AndroidAbi::from_device_abi(&t))
    }

    /// Install (or replace) the package `apk` on `serial`, keeping the application's data.
    pub fn install(&self, serial: &str, apk: &Path) -> Result<(), String> {
        let apk = apk.to_string_lossy();
        let text = self.run(Some(serial), &["install", "-r", &apk])?;
        if text.contains("Success") {
            Ok(())
        } else {
            Err(format!("the package was not installed: {}", text.trim()))
        }
    }

    /// Start the application `app_id` on `serial` (Tauri's activity is `<app id>.MainActivity`).
    pub fn launch(&self, serial: &str, app_id: &str) -> Result<(), String> {
        let component = format!("{app_id}/.MainActivity");
        let text = self.run(Some(serial), &["shell", "am", "start", "-n", &component])?;
        if text.contains("Error") {
            Err(format!("the application did not start: {}", text.trim()))
        } else {
            Ok(())
        }
    }

    /// Stop the application `app_id` on `serial`, so the next start loads the new package.
    pub fn stop(&self, serial: &str, app_id: &str) -> Result<(), String> {
        self.run(Some(serial), &["shell", "am", "force-stop", app_id]).map(|_| ())
    }

    /// The application's log from now on: a running `adb logcat` whose output is read line by
    /// line (stop it with `kill`).
    pub fn logcat(&self, serial: &str) -> Result<Child, String> {
        let mut c = self.command(Some(serial));
        c.args(["logcat", "-T", "1", "-v", "brief"]).args(LOG_TAGS).stdout(Stdio::piped()).stderr(Stdio::null());
        c.spawn().map_err(|e| format!("adb logcat could not be started ({e})"))
    }
}
