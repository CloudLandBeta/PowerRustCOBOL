// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Android devices and emulators** a Mobile application runs on (WASM-012 T16, T20),
//! reached with the SDK's `adb` and `emulator`: the ones connected, the emulators that can be
//! started and starting one, the processor of a device (the package is built for it), installing a
//! package, starting the application, and its log.
//!
//! What the tools print is read by small parsers ([`parse_devices`], [`parse_avds`],
//! [`is_application_log`]…) so the answers are tested from recorded output; the commands
//! themselves need a device.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

/// The `adb` arguments that start the application `app_id` as the phone's launcher does: Tauri's
/// activity (`<app id>.MainActivity`) with the launcher's intent (`MAIN`, `LAUNCHER`), so a run
/// from the IDE starts the task a tap on the icon would, and a later tap on the icon finds it.
/// (Back on the first screen closes the application either way on Android 17: Tauri's own Back
/// handler calls the activity's, and the shell then ends the process cleanly.)
pub fn launch_args(app_id: &str) -> Vec<String> {
    let component = format!("{app_id}/.MainActivity");
    ["shell", "am", "start", "-a", "android.intent.action.MAIN", "-c", "android.intent.category.LAUNCHER", "-n", &component]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

/// What the IDE's Output shows of `line` (of `adb logcat -v brief`), or `None` when it is not the
/// application's ([`is_application_log`]). A console line is shown as the program wrote it — a
/// DISPLAY reads as it does on the desktop, without the log's level, tag, process number and the
/// web view's `File: … - Line … - Msg:` — and so is a line of the shell; a crash keeps Android's
/// whole line, so it reads as the crash it is.
pub fn application_log(line: &str) -> Option<&str> {
    if !is_application_log(line) {
        return None;
    }
    if line.starts_with("E/AndroidRuntime") {
        return Some(line);
    }
    let text = line.split_once("): ").map_or(line, |(_, t)| t);
    Some(text.strip_prefix("File: ").and_then(|t| t.split_once(" - Msg: ")).map_or(text, |(_, m)| m))
}

/// Is the activity of the application `app_id` among the activities in `dumpsys activity
/// activities`'s answer? It is from the moment a start is taken (before the process has even been
/// named), and not after the application is stopped.
pub fn has_activity(dumpsys: &str, app_id: &str) -> bool {
    let record = format!(" {app_id}/.MainActivity ");
    dumpsys.lines().any(|l| l.contains("ActivityRecord{") && l.contains(&record))
}

/// The Linux user id of the application `app_id` in `pm list packages -U <app_id>`'s answer
/// (`package:<id> uid:<n>`; the command also lists every package whose id contains `app_id`).
pub fn parse_package_uid(text: &str, app_id: &str) -> Option<u32> {
    text.lines().find_map(|l| {
        let (package, uid) = l.trim().strip_prefix("package:")?.split_once(" uid:")?;
        (package == app_id).then(|| uid.trim().parse().ok()).flatten()
    })
}

/// The arguments of `adb logcat` that follow the application's log from now on: its tags
/// ([`LOG_TAGS`]) and, when its user id is known, only its own processes. Without the user id a
/// crash of any application on the device (`E/AndroidRuntime`) would read as this one's; with it,
/// a new process of the application (started again from its icon) is still followed.
pub fn logcat_args(uid: Option<u32>) -> Vec<String> {
    let mut args: Vec<String> = ["logcat", "-T", "1", "-v", "brief"].iter().map(|s| (*s).to_owned()).collect();
    args.extend(uid.map(|u| format!("--uid={u}")));
    args.extend(LOG_TAGS.iter().map(|s| (*s).to_owned()));
    args
}

/// The emulators (Android Virtual Devices) in `emulator -list-avds`'s answer. An emulator's name
/// is made of letters, digits, `.`, `_` and `-`; any other line (a note the emulator prints about
/// itself) is not one.
pub fn parse_avds(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .map(str::to_owned)
        .collect()
}

/// The emulator's name in `adb emu avd name`'s answer (the name, then `OK`).
pub fn parse_avd_name(text: &str) -> Option<String> {
    let first = text.lines().next()?.trim();
    (!first.is_empty() && first != "OK" && !first.starts_with("KO")).then(|| first.to_owned())
}

/// Has the device finished starting? (`getprop sys.boot_completed` answers `1`; before, nothing.)
pub fn boot_completed(getprop: &str) -> bool {
    getprop.trim() == "1"
}

/// Does the device's package service answer? (`service check package`: `Service package: found`,
/// or `… not found` while Android starts or restarts its system.)
pub fn package_service_found(answer: &str) -> bool {
    answer.trim().ends_with(": found")
}

/// What a Run can go to, in the order it prefers them: the devices and emulators running now
/// (`ready`, their serial numbers), then the emulators that can be started (`avds`, their names),
/// less the ones already running (`running_avds`).
pub fn targets(ready: &[String], running_avds: &[String], avds: &[String]) -> Vec<String> {
    ready.iter().chain(avds.iter().filter(|a| !running_avds.contains(a))).cloned().collect()
}

/// The arguments that start the emulator `avd`: its own window, as the Device Manager of Android
/// Studio starts it (a saved state is resumed when there is one, so a second start is quick).
pub fn emulator_args(avd: &str) -> Vec<String> {
    vec!["-avd".to_owned(), avd.to_owned()]
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

    /// The emulator `serial` is (`adb emu avd name`); `None` for a phone.
    pub fn avd_name(&self, serial: &str) -> Option<String> {
        if !serial.starts_with("emulator-") {
            return None;
        }
        self.run(Some(serial), &["emu", "avd", "name"]).ok().and_then(|t| parse_avd_name(&t))
    }

    /// Can `serial` take a package now? The system has finished starting ([`boot_completed`]) and
    /// its package service answers ([`package_service_found`]). The first alone is not enough: it
    /// stays `1` while Android restarts its system after a hang (an emulator starved by a busy
    /// computer), and an install then fails with "Can't find service: package".
    pub fn ready_to_install(&self, serial: &str) -> bool {
        self.run(Some(serial), &["shell", "getprop", "sys.boot_completed"]).is_ok_and(|t| boot_completed(&t))
            && self.run(Some(serial), &["shell", "service", "check", "package"]).is_ok_and(|t| package_service_found(&t))
    }

    /// Wait until `serial` can take a package ([`Adb::ready_to_install`]), or why not after
    /// `timeout`. At once for a device that is ready.
    pub fn wait_until_ready(&self, serial: &str, timeout: Duration) -> Result<(), String> {
        let t0 = Instant::now();
        while !self.ready_to_install(serial) {
            if t0.elapsed() >= timeout {
                return Err(format!("{serial} was not ready to install in {} s (its system was starting)", timeout.as_secs()));
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        Ok(())
    }

    /// Wait until the emulator `avd` is listed and can take a package: its serial number, or why
    /// not after `timeout`.
    pub fn wait_for_avd(&self, avd: &str, timeout: Duration) -> Result<String, String> {
        let t0 = Instant::now();
        loop {
            let started = self.devices().into_iter().filter(Device::ready).find(|d| self.avd_name(&d.serial).as_deref() == Some(avd));
            if let Some(d) = started.filter(|d| self.ready_to_install(&d.serial)) {
                return Ok(d.serial);
            }
            if t0.elapsed() >= timeout {
                return Err(format!("the emulator {avd} did not finish starting in {} s", timeout.as_secs()));
            }
            std::thread::sleep(Duration::from_secs(2));
        }
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

    /// Start the application `app_id` on `serial` as a tap on its icon does ([`launch_args`]). A
    /// device that has just started refuses a start for a few seconds (measured: an emulator that
    /// resumed its saved state took a start before its user was unlocked); a refused start is
    /// asked again every 2 s until `patience` has passed, then its reason is answered. A start that
    /// answered a failure but was taken (measured: a busy emulator answered after 18 s) is not
    /// asked again: a second start replaces the application's activity, and that ends a Tauri
    /// application ([`has_activity`]).
    pub fn launch(&self, serial: &str, app_id: &str, patience: Duration) -> Result<(), String> {
        let args = launch_args(app_id);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let t0 = Instant::now();
        loop {
            let why = match self.run(Some(serial), &args) {
                Ok(text) if !text.contains("Error") => return Ok(()),
                Ok(text) => format!("the application did not start: {}", text.trim()),
                Err(why) => why,
            };
            let taken = self.run(Some(serial), &["shell", "dumpsys", "activity", "activities"]).is_ok_and(|t| has_activity(&t, app_id));
            if taken {
                return Ok(());
            }
            if t0.elapsed() >= patience {
                return Err(why);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    /// Stop the application `app_id` on `serial`; before an install, so Android has no running
    /// application to cover and start again when the update is done.
    pub fn stop(&self, serial: &str, app_id: &str) -> Result<(), String> {
        self.run(Some(serial), &["shell", "am", "force-stop", app_id]).map(|_| ())
    }

    /// The Linux user id of the installed application `app_id` on `serial` ([`parse_package_uid`]).
    pub fn uid(&self, serial: &str, app_id: &str) -> Option<u32> {
        self.run(Some(serial), &["shell", "pm", "list", "packages", "-U", app_id]).ok().and_then(|t| parse_package_uid(&t, app_id))
    }

    /// The log of the application whose user id is `uid` from now on ([`logcat_args`]): a running
    /// `adb logcat` whose output is read line by line (stop it with `kill`).
    pub fn logcat(&self, serial: &str, uid: Option<u32>) -> Result<Child, String> {
        let mut c = self.command(Some(serial));
        c.args(logcat_args(uid)).stdout(Stdio::piped()).stderr(Stdio::null());
        c.spawn().map_err(|e| format!("adb logcat could not be started ({e})"))
    }
}

/// The SDK's `emulator`.
#[derive(Debug, Clone)]
pub struct Emulator {
    pub path: PathBuf,
}

impl Emulator {
    /// `emulator` of the Android SDK [`android_sdk`] finds.
    pub fn find() -> Option<Self> {
        let sdk = android_sdk()?;
        Self::in_sdk(&sdk)
    }

    /// `emulator` of the SDK at `sdk`.
    pub fn in_sdk(sdk: &Path) -> Option<Self> {
        let path = sdk.join("emulator").join(if cfg!(windows) { "emulator.exe" } else { "emulator" });
        path.is_file().then_some(Self { path })
    }

    /// The emulators set up on this computer (Android Studio's Device Manager creates them).
    pub fn avds(&self) -> Vec<String> {
        Command::new(&self.path)
            .arg("-list-avds")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map(|o| parse_avds(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }

    /// Start the emulator `avd` ([`emulator_args`]) and return at once; [`Adb::wait_for_avd`] waits
    /// for it. It is the developer's from then on: it goes on running when the IDE ends (on Unix in
    /// its own process group, so an interrupt of the IDE's terminal does not reach it).
    pub fn start(&self, avd: &str) -> Result<(), String> {
        let mut c = Command::new(&self.path);
        c.args(emulator_args(avd)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        let mut child = c.spawn().map_err(|e| format!("the emulator could not be started ({e})"))?;
        // Reap it when it ends, so it does not linger as a finished process while the IDE runs.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
