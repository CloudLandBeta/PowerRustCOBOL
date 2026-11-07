// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Android device of a Mobile project** (WASM-012 T17, T20): the IDE's [`DeviceRunner`].
//!
//! * A **Run** (the Preview) of a Mobile project for Android, with an emulator running or a phone
//!   connected, builds the development package from the bundle the Preview serves, installs it on
//!   that device, starts it and shows the application's log (the page's console, the shell's lines,
//!   a crash) in the Output; without them the Preview opens in the browser at a phone's width, as
//!   before.
//! * A **Build** goes on into the package (`dist/mobile/android/<Name>-debug.apk`).
//!
//! Both run the Tauri command line and Gradle, which take seconds when warm and a few minutes the
//! first time: never on the UI thread. Every line of the tools goes to `package.log` in the staging
//! folder; the Output shows the steps, the errors and the result.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};

use cobolt_compiler::mobile_build::{self as mb, AndroidAbi, PackageRequest, Profile, Toolchain};
use cobolt_compiler::mobile_devices::{self as md, Adb};
use cobolt_compiler::project_kind::MobilePlatform;

use super::web_preview::{device_tool, DeviceRunner};
use crate::i18n::Tr;

/// The most lines kept for the Output between two polls.
const MAX_QUEUED: usize = 2000;

#[derive(Default)]
struct State {
    lines: VecDeque<String>,
    busy: bool,
    /// The `adb logcat` of the application started last.
    logcat: Option<Child>,
}

/// One Mobile project's Android device runner.
pub struct MobileDeviceRunner {
    job: Job,
}

/// What a worker thread needs.
#[derive(Clone)]
struct Job {
    manifest: PathBuf,
    tr: Tr,
    state: Arc<Mutex<State>>,
}

impl MobileDeviceRunner {
    pub fn new(manifest: PathBuf, tr: Tr) -> Self {
        Self { job: Job { manifest, tr, state: Arc::default() } }
    }

    /// Take the worker slot: `Err` (the reason, in the developer's language) when it is taken.
    fn claim(&self) -> Result<(), String> {
        let mut st = self.job.state.lock().unwrap_or_else(|p| p.into_inner());
        if st.busy {
            return Err(self.job.tr.pk_dev_busy.to_owned());
        }
        st.busy = true;
        Ok(())
    }
}

impl Drop for MobileDeviceRunner {
    fn drop(&mut self) {
        if let Some(mut c) = self.job.state.lock().unwrap_or_else(|p| p.into_inner()).logcat.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// A tool's line worth showing in the Output: a step, an error, the end. The rest is in
/// `package.log`.
fn worth_showing(line: &str) -> bool {
    let l = line.trim_start();
    ["$ cargo", "Staging", "Package:", "error", "Error", "BUILD ", "FAILURE", "What went wrong", "Finished"].iter().any(|p| l.starts_with(p))
}

fn fill(template: &str, values: &[&str]) -> String {
    values.iter().fold(template.to_owned(), |t, v| t.replacen("{}", v, 1))
}

impl Job {
    fn say(&self, line: impl Into<String>) {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if st.lines.len() >= MAX_QUEUED {
            st.lines.pop_front();
        }
        st.lines.push_back(line.into());
    }

    fn done(&self) {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).busy = false;
    }

    /// Build the development package for `abi`: the package and the application id.
    fn package_for(&self, abi: AndroidAbi) -> Result<(mb::Package, String), String> {
        let view = cobolt_compiler::project_manifest_view(&self.manifest)?;
        let name = mb::application_name(&view.name);
        let staging = cobolt_compiler::build_staging_dir(None, &format!("{}-android", mb::crate_name(&name)));
        let req = PackageRequest::from_manifest(&self.manifest, staging, Profile::Debug, abi)?;
        let tools = Toolchain::detect(abi).map_err(|missing| missing.into_iter().map(|m| m.how).collect::<Vec<_>>().join("; "))?;
        let _ = std::fs::create_dir_all(&req.staging_dir);
        let mut file = std::fs::File::create(req.staging_dir.join("package.log")).ok();
        let mut log = |line: &str| {
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{line}");
            }
            if worth_showing(line) {
                self.say(line);
            }
        };
        let package = mb::build_android(&req, &tools, None, &mut log)?;
        Ok((package, req.settings.app_id.trim().to_owned()))
    }

    fn say_built(&self, p: &mb::Package) {
        let mb_size = format!("{:.1}", p.bytes as f64 / 1_048_576.0);
        self.say(fill(self.tr.pk_pkg_built, &[&p.path.to_string_lossy(), &mb_size]));
    }

    /// Build, install, start and follow the application on `device`.
    fn run_on(&self, device: &str) -> Result<(), String> {
        let adb = Adb::find().ok_or_else(|| "adb was not found".to_owned())?;
        let abi = adb.abi(device).unwrap_or(AndroidAbi::Aarch64);
        let (package, app_id) = self.package_for(abi)?;
        self.say_built(&package);
        self.say(fill(self.tr.pk_dev_installing, &[device]));
        adb.install(device, &package.path)?;
        let _ = adb.stop(device, &app_id);
        adb.launch(device, &app_id)?;
        self.say(fill(self.tr.pk_dev_started, &[device]));
        let mut child = adb.logcat(device)?;
        if let Some(out) = child.stdout.take() {
            let job = self.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if md::is_application_log(&line) {
                        job.say(line);
                    }
                }
            });
        }
        let old = self.state.lock().unwrap_or_else(|p| p.into_inner()).logcat.replace(child);
        if let Some(mut c) = old {
            let _ = c.kill();
            let _ = c.wait();
        }
        Ok(())
    }
}

impl DeviceRunner for MobileDeviceRunner {
    fn detect(&self, platform: MobilePlatform) -> Result<(), String> {
        // Only what can be checked without running a tool: this runs on the UI thread. The rest
        // (the Tauri command line, the NDK, Java) is checked by the run, which says what is missing.
        match platform {
            MobilePlatform::Ios => Err(device_tool(platform).to_owned()),
            MobilePlatform::Android => Adb::find().map(|_| ()).ok_or_else(|| device_tool(platform).to_owned()),
        }
    }

    fn list(&self, platform: MobilePlatform) -> Vec<String> {
        match (platform, Adb::find()) {
            (MobilePlatform::Android, Some(adb)) => adb.devices().into_iter().filter(md::Device::ready).map(|d| d.serial).collect(),
            _ => Vec::new(),
        }
    }

    fn run(&self, device: &str, _dev_url: &str) -> Result<(), String> {
        self.claim()?;
        let job = self.job.clone();
        let device = device.to_owned();
        std::thread::spawn(move || {
            if let Err(why) = job.run_on(&device) {
                job.say(fill(job.tr.pk_dev_failed, &[&device, &why]));
            }
            job.done();
        });
        Ok(())
    }

    fn log(&self) -> Vec<String> {
        self.job.state.lock().unwrap_or_else(|p| p.into_inner()).lines.drain(..).collect()
    }

    fn package(&self) -> Result<(), String> {
        self.claim()?;
        let job = self.job.clone();
        std::thread::spawn(move || {
            match job.package_for(AndroidAbi::Aarch64) {
                Ok((p, _)) => job.say_built(&p),
                Err(why) => job.say(fill(job.tr.pk_pkg_failed, &[&why])),
            }
            job.done();
        });
        Ok(())
    }

    fn busy(&self) -> bool {
        self.job.state.lock().unwrap_or_else(|p| p.into_inner()).busy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_shows_the_steps_and_the_errors_of_a_package_build_not_every_line() {
        let shown = ["$ cargo tauri android build --ci --debug --apk --target aarch64", "Staging the mobile shell in /x", "error[E0425]: cannot find value", "BUILD SUCCESSFUL in 7s", "    Finished 1 APK at:", "Package: /p/dist/mobile/android/A-debug.apk (12.0 MB)"];
        let hidden = ["   Compiling tauri v2.12.5", "> Task :app:mergeDebugAssets", "        Info Patching"];
        assert!(shown.iter().all(|l| worth_showing(l)), "{shown:?}");
        assert!(!hidden.iter().any(|l| worth_showing(l)), "{hidden:?}");
        let tr = crate::i18n::Language::English.tr();
        let r = MobileDeviceRunner::new(PathBuf::from("/nowhere/A.project.toml"), tr);
        r.claim().unwrap();
        assert_eq!(r.package(), Err(tr.pk_dev_busy.to_owned()), "one package build at a time");
        assert!(r.busy());
        println!("mobile runner: {} step lines shown, {} tool lines kept for package.log only, a second build while one runs refused", shown.len(), hidden.len());
    }

    /// The IDE's Run of a Mobile project, end to end, without the IDE's window: the runner the
    /// Preview is given builds `examples/PowerMobileHello` for the first device `adb` sees,
    /// installs it, starts it and follows its log. Run it with an emulator started and the
    /// example's bundle built (`rcrun mcp` `build`, or the IDE's Build):
    /// `cargo test -p cobolt-ide --bin cobolt-ide the_example_runs_on_the_first_android_device -- --ignored --nocapture`
    #[test]
    #[ignore = "needs an Android emulator or phone, the SDK, the NDK, Java 17 to 21 and the Tauri CLI"]
    fn the_example_runs_on_the_first_android_device() {
        use std::time::{Duration, Instant};
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerMobileHello/PowerMobileHello.project.toml");
        let tr = crate::i18n::Language::English.tr();
        let r = MobileDeviceRunner::new(manifest, tr);
        r.detect(MobilePlatform::Android).expect("adb is found");
        let device = r.list(MobilePlatform::Android).into_iter().next().expect("an emulator or a phone is connected");
        let t0 = Instant::now();
        r.run(&device, "").expect("the worker starts");
        let mut lines = Vec::new();
        while r.busy() && t0.elapsed() < Duration::from_secs(900) {
            lines.extend(r.log());
            std::thread::sleep(Duration::from_millis(250));
        }
        let built_in = t0.elapsed();
        // The application's own lines (the page's console, the shell) arrive after the start.
        std::thread::sleep(Duration::from_secs(10));
        lines.extend(r.log());
        for l in &lines {
            println!("  | {l}");
        }
        let started = fill(tr.pk_dev_started, &[&device]);
        assert!(lines.iter().any(|l| l == &started), "the application was not started on {device}");
        let after = lines.iter().position(|l| l == &started).map_or(0, |i| lines.len() - i - 1);
        println!("mobile runner on {device}: built, installed and started in {:.1} s; {after} application log lines in the next 10 s", built_in.as_secs_f64());
    }
}
