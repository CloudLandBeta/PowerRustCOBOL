// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Android Studio project of a Mobile project, made from the IDE** (WASM-012 R16, R17;
//! WASM-001 R19).
//!
//! * [`NativeJob`] makes it on a thread of its own ([`cobolt_compiler::mobile_native::create_android`]:
//!   the Tauri command line and the Android tools take seconds when warm, a minute the first
//!   time), from the Mobile settings the developer sees — so an Application id typed in Project
//!   Settings and not saved yet is the one the project gets. Every line of the tools goes to
//!   `native.log` in the project's build folder.
//! * [`NativeStep`] is New Project's step of its own for a Mobile project for Android: *Create* or
//!   *Not now*. Not now leaves the project without it; Project Settings, Mobile makes it later.
//! * [`Studio`] is *Open in Android Studio*: the Tauri command line the project's Gradle build talks
//!   to while the developer works in Android Studio, kept by the app.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, SendError, TryRecvError};

use cobolt_compiler::mobile_build::{self as mb, AndroidAbi, PackageRequest, Profile};
use cobolt_compiler::mobile_native::{self as mn, Created};
use cobolt_compiler::project_kind::MobileSettings;
use eframe::egui;

use crate::i18n::Tr;
use crate::theme::Theme;

/// What making the Android Studio project came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The project, in this folder (the one Android Studio opens).
    Ready(PathBuf),
    /// Only its sources: this computer lacks these tools (named, with how to install each).
    SourcesOnly(Vec<String>),
    /// It could not be made: why.
    Failed(String),
}

impl Outcome {
    /// The line the developer reads, in their language.
    pub fn text(&self, tr: &Tr) -> String {
        match self {
            Outcome::Ready(dir) => tr.pk_native_done.replacen("{}", &dir.to_string_lossy(), 1),
            Outcome::SourcesOnly(missing) => tr.pk_native_sources_only.replacen("{}", &missing.join("; "), 1),
            Outcome::Failed(why) => tr.pk_native_failed.replacen("{}", why, 1),
        }
    }

    /// The Android Studio project is there.
    pub fn is_ready(&self) -> bool {
        matches!(self, Outcome::Ready(_))
    }
}

/// The request that makes the Android Studio project of the project at `manifest`, as saved.
pub fn request_from_manifest(manifest: &Path) -> Result<PackageRequest, String> {
    PackageRequest::from_manifest(manifest, PathBuf::new(), Profile::Debug, AndroidAbi::Aarch64)
}

/// The request that makes the Android Studio project of the project in `project_dir` named
/// `name` (with or without `.project`), at `version`, with `settings` as its Mobile settings —
/// what Project Settings shows, saved or not.
pub fn request_from(project_dir: &Path, name: &str, version: &str, settings: MobileSettings) -> PackageRequest {
    PackageRequest {
        project_dir: project_dir.to_path_buf(),
        project_name: mb::application_name(name),
        version: version.to_owned(),
        settings,
        bundle_dir: project_dir.join(mb::BUNDLE_SUBDIR),
        // The native project's own folder is its staging folder (`create_android` sets it).
        staging_dir: PathBuf::new(),
        profile: Profile::Debug,
        abi: AndroidAbi::Aarch64,
    }
}

/// Make the Android Studio project `req` describes. Blocking: run it off the UI thread.
pub fn create(req: &PackageRequest) -> Outcome {
    let log_dir = cobolt_compiler::build_staging_dir(None, &format!("{}-android", req.crate_name()));
    let _ = std::fs::create_dir_all(&log_dir);
    let mut file = std::fs::File::create(log_dir.join("native.log")).ok();
    let mut log = |line: &str| {
        if let Some(f) = file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    };
    match mn::create_android(req, &mut log) {
        Ok(Created::Ready(dir)) => Outcome::Ready(dir),
        Ok(Created::SourcesOnly(missing)) => Outcome::SourcesOnly(missing.into_iter().map(|m| m.how).collect()),
        Err(why) => Outcome::Failed(why),
    }
}

/// The making of an Android Studio project, on a thread of its own.
#[derive(Default)]
pub struct NativeJob {
    running: Option<Receiver<Outcome>>,
    /// What the last one came to.
    pub last: Option<Outcome>,
}

impl NativeJob {
    /// Start making it; `ctx` is repainted when it is done.
    pub fn start(&mut self, ctx: &egui::Context, req: PackageRequest) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(create(&req));
            ctx.request_repaint();
        });
        self.running = Some(rx);
        self.last = None;
    }

    pub fn running(&self) -> bool {
        self.running.is_some()
    }

    /// The outcome, once, on the frame it arrives.
    pub fn poll(&mut self) -> Option<Outcome> {
        let outcome = self.running.as_ref()?.try_recv().ok()?;
        self.running = None;
        self.last = Some(outcome.clone());
        Some(outcome)
    }
}

/// What the Tauri command line for Android Studio is doing, for one project (Open in Android
/// Studio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StudioState {
    /// Not started for this project.
    Idle,
    /// Bringing the project up to date and starting it.
    Starting,
    /// Running: Android Studio's builds of the project work.
    Running,
    /// It could not start: why.
    Failed(String),
    /// It ended by itself: how.
    Ended(String),
    /// Stopped from Project Settings.
    Stopped,
}

impl StudioState {
    /// The note under the row, in the developer's language, or none; `log` is the log's path.
    pub fn text(&self, tr: &Tr, log: &str) -> Option<String> {
        match self {
            StudioState::Idle => None,
            StudioState::Starting | StudioState::Running => Some(tr.pk_native_opening.replacen("{}", log, 1)),
            StudioState::Failed(why) => Some(tr.pk_native_open_failed.replacen("{}", why, 1).replacen("{}", log, 1)),
            StudioState::Ended(how) => Some(tr.pk_native_open_ended.replacen("{}", how, 1).replacen("{}", log, 1)),
            StudioState::Stopped => Some(tr.pk_native_stopped.to_owned()),
        }
    }

    /// Starting or running.
    pub fn is_live(&self) -> bool {
        matches!(self, StudioState::Starting | StudioState::Running)
    }

    /// It failed or ended by itself.
    pub fn is_problem(&self) -> bool {
        matches!(self, StudioState::Failed(_) | StudioState::Ended(_))
    }
}

/// The Tauri command line Android Studio's builds of a project talk to, started by Open in Android
/// Studio ([`mn::open_android_studio`]). The app keeps the one there is, so closing Project
/// Settings leaves it running; Stop, the next Open and closing the IDE end it.
#[derive(Default)]
pub struct Studio {
    starting: Option<Receiver<Result<Child, String>>>,
    child: Option<Child>,
    /// The project it is for.
    project: Option<PathBuf>,
    /// The tools' lines (`android-studio.log` in the project's build folder).
    log: Option<PathBuf>,
    /// How the last one ended.
    ended: Option<StudioState>,
}

/// The app's [`Studio`], lent to Project Settings.
pub type SharedStudio = Rc<RefCell<Studio>>;

impl Studio {
    /// Open the project `req` describes in Android Studio, stopping the one running; `ctx` is
    /// repainted once it has started (or could not). The request's staging folder becomes the
    /// project's package build folder, which holds the log and the Rust build the native project's
    /// starts from.
    pub fn open(&mut self, ctx: &egui::Context, mut req: PackageRequest) {
        self.stop();
        let build_dir = cobolt_compiler::build_staging_dir(None, &format!("{}-android", req.crate_name()));
        let log = build_dir.join("android-studio.log");
        req.staging_dir = build_dir;
        self.project = Some(req.project_dir.clone());
        self.log = Some(log.clone());
        self.ended = None;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Stopped while it started: the receiver is gone, so the process it started is ended here.
            if let Err(SendError(Ok(mut child))) = tx.send(mn::open_android_studio(&req, &log)) {
                let _ = child.kill();
                let _ = child.wait();
            }
            ctx.request_repaint();
        });
        self.starting = Some(rx);
    }

    fn poll(&mut self) {
        if let Some(rx) = &self.starting {
            match rx.try_recv() {
                Ok(Ok(child)) => {
                    self.child = Some(child);
                    self.starting = None;
                }
                Ok(Err(why)) => {
                    self.ended = Some(StudioState::Failed(why));
                    self.starting = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.ended = Some(StudioState::Failed("the start was interrupted".to_owned()));
                    self.starting = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(child) = self.child.as_mut() {
            let ended = match child.try_wait() {
                Ok(None) => return,
                Ok(Some(status)) => status.to_string(),
                Err(e) => e.to_string(),
            };
            self.ended = Some(StudioState::Ended(ended));
            self.child = None;
        }
    }

    /// What it is doing for the project in `project_dir`.
    pub fn state(&mut self, project_dir: &Path) -> StudioState {
        self.poll();
        if self.project.as_deref() != Some(project_dir) {
            StudioState::Idle
        } else if self.starting.is_some() {
            StudioState::Starting
        } else if self.child.is_some() {
            StudioState::Running
        } else {
            self.ended.clone().unwrap_or(StudioState::Idle)
        }
    }

    /// The log of the last one.
    pub fn log(&self) -> Option<&Path> {
        self.log.as_deref()
    }

    /// End it, if it is starting or running.
    pub fn stop(&mut self) {
        let live = self.starting.take().is_some() | self.child.is_some();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if live {
            self.ended = Some(StudioState::Stopped);
        }
    }
}

impl Drop for Studio {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The record `[mobile.native]` keeps of the Android Studio project once it is made (R18).
pub fn record_android(settings: &mut MobileSettings) {
    settings.native.android = mn::ANDROID_DIR.to_owned();
}

/// Whether New Project offers the native-project step for a project just made (WASM-001 R19):
/// only a Mobile project for Android has one. A Web or Desktop project has no `[mobile]`
/// settings; an iOS project's Xcode project is not made by this version.
pub fn offers_step(mobile: Option<&MobileSettings>) -> bool {
    mobile.is_some_and(|m| m.platform == cobolt_compiler::project_kind::MobilePlatform::Android)
}

/// What New Project's step asks the app to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StepAction {
    /// *Create*: make it for this project.
    pub create: Option<PathBuf>,
}

const STEP_W: f32 = 520.0;
const STEP_H: f32 = 250.0;

/// New Project's step for a Mobile project for Android (WASM-001 R19): asked once, after the
/// project is made and opened.
pub struct NativeStep {
    /// The project's manifest.
    manifest: PathBuf,
    /// The project's name, as the step says it.
    name: String,
    size: egui::Vec2,
}

impl NativeStep {
    pub fn new(manifest: PathBuf, name: &str) -> Self {
        Self { manifest, name: mb::application_name(name), size: egui::vec2(STEP_W, STEP_H) }
    }

    /// The step's body: what it makes, where, and what it needs.
    pub fn body(&self, tr: &Tr) -> String {
        let dir = self.manifest.parent().map(|d| mn::android_root(d).to_string_lossy().into_owned()).unwrap_or_default();
        tr.pk_native_step_body.replacen("{}", &self.name, 1).replacen("{}", &dir, 1)
    }

    /// One frame: `Some` once the developer answered (the step closes).
    pub fn show(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr) -> Option<StepAction> {
        let mut answer = None;
        let mut open = true;
        egui::Window::new(egui::RichText::new(tr.pk_native_step_title).size(18.0).strong())
            .id(egui::Id::new("native_project_step"))
            .open(&mut open)
            .collapsible(false)
            // The size is the step's own (a window never resizes itself): nothing in it grows.
            .resizable(false)
            .fixed_size(self.size)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let inner_w = (self.size.x - ui.style().spacing.window_margin.sum().x).max(120.0);
                ui.set_width(inner_w);
                ui.add(egui::Label::new(egui::RichText::new(self.body(tr)).color(theme.text_bright)).wrap());
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.pk_native_create).clicked() {
                        answer = Some(StepAction { create: Some(self.manifest.clone()) });
                    }
                    if ui.button(tr.pk_native_step_skip).clicked() {
                        answer = Some(StepAction::default());
                    }
                });
            });
        if !open && answer.is_none() {
            answer = Some(StepAction::default());
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn the_step_and_the_outcomes_read_in_six_languages_012() {
        let step = NativeStep::new(PathBuf::from("/p/Orders/Orders.project.toml"), "Orders.project");
        let outcomes = [
            Outcome::Ready(PathBuf::from("/p/Orders/android/src-tauri/gen/android")),
            Outcome::SourcesOnly(vec!["the Android SDK was not found".into()]),
            Outcome::Failed("tauri android init failed".into()),
        ];
        let mut lines = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            let body = step.body(&tr);
            assert!(body.contains("Orders") && body.contains("/p/Orders/android") && !body.contains("{}"), "{lang:?}: {body}");
            for o in &outcomes {
                let t = o.text(&tr);
                assert!(!t.contains("{}"), "{lang:?}: {t}");
                lines += 1;
            }
            assert!(outcomes[0].text(&tr).contains("gen/android"));
            for s in [tr.pk_native_step_title, tr.pk_native_create, tr.pk_native_recreate, tr.pk_native_step_skip, tr.pk_set_native_android] {
                assert!(!s.trim().is_empty(), "{lang:?}");
            }
        }
        assert!(outcomes[0].is_ready() && !outcomes[1].is_ready() && !outcomes[2].is_ready());
        println!("native project step: 6 languages, {lines} outcome lines, every placeholder filled");
    }

    #[test]
    fn open_in_android_studio_says_what_it_is_doing_in_six_languages_012() {
        let states = [
            StudioState::Starting,
            StudioState::Running,
            StudioState::Failed("the Android NDK was not found".into()),
            StudioState::Ended("exit status: 1".into()),
            StudioState::Stopped,
        ];
        let log = "/caches/builds/cobolt-build-orders-android/android-studio.log";
        let mut lines = 0;
        for lang in Language::ALL {
            let tr = lang.tr();
            assert_eq!(StudioState::Idle.text(&tr, log), None);
            for s in &states {
                let t = s.text(&tr, log).unwrap_or_default();
                assert!(!t.is_empty() && !t.contains("{}"), "{lang:?} {s:?}: {t}");
                if *s != StudioState::Stopped {
                    assert!(t.contains(log), "{lang:?} {s:?}: the log is named");
                }
                lines += 1;
            }
            assert!(states[2].text(&tr, log).unwrap().contains("NDK") && states[3].text(&tr, log).unwrap().contains("exit status: 1"));
            for b in [tr.pk_native_open, tr.pk_native_stop] {
                assert!(!b.trim().is_empty(), "{lang:?}");
            }
        }
        assert!(states[0].is_live() && states[1].is_live() && !states[4].is_live());
        assert!(states[2].is_problem() && states[3].is_problem() && !states[1].is_problem() && !states[4].is_problem());
        println!("open in Android Studio: 6 languages, {lines} state lines, the log named in every one but Stopped");
    }

    /// The process is the project's: another project sees nothing; Stop ends it and says so; a
    /// process that ends by itself is reported with its status; dropping the Studio ends it too.
    #[cfg(unix)]
    #[test]
    fn the_studio_ends_its_process_on_stop_and_on_drop_012() {
        let dir = PathBuf::from("/p/Orders");
        let spawn = || std::process::Command::new("sleep").arg("30").spawn().expect("sleep");
        let studio_with = |child: Child| {
            let mut s = Studio::default();
            s.project = Some(dir.clone());
            s.child = Some(child);
            s
        };
        let mut studio = studio_with(spawn());
        studio.log = Some(PathBuf::from("/l/android-studio.log"));
        assert_eq!(studio.state(&dir), StudioState::Running);
        assert_eq!(studio.state(Path::new("/p/Other")), StudioState::Idle, "another project's page sees none");
        let pid = studio.child.as_ref().map(Child::id).unwrap();
        studio.stop();
        assert_eq!(studio.state(&dir), StudioState::Stopped);
        assert!(studio.child.is_none() && studio.log() == Some(Path::new("/l/android-studio.log")));
        let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
        assert!(!alive, "the process is gone");

        let quick = std::process::Command::new("sh").args(["-c", "exit 3"]).spawn().unwrap();
        let mut studio = studio_with(quick);
        let t = std::time::Instant::now();
        while studio.state(&dir).is_live() && t.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(matches!(studio.state(&dir), StudioState::Ended(ref how) if how.contains('3')), "{:?}", studio.state(&dir));

        let held = spawn();
        let pid = held.id();
        drop(studio_with(held));
        let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
        assert!(!alive, "closing the IDE ends it");
        println!("studio: running, idle for another project, stopped (process gone), ended by itself (exit 3), ended with the app");
    }

    #[test]
    fn a_made_project_is_recorded_in_the_mobile_settings_012() {
        let mut m = MobileSettings::new(cobolt_compiler::project_kind::MobilePlatform::Android, cobolt_compiler::project_kind::DeviceClass::Phone);
        assert!(m.native.is_default());
        record_android(&mut m);
        assert_eq!(m.native.android, "android");
    }
}
