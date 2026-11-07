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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

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

/// The record `[mobile.native]` keeps of the Android Studio project once it is made (R18).
pub fn record_android(settings: &mut MobileSettings) {
    settings.native.android = mn::ANDROID_DIR.to_owned();
}

/// The folder Android Studio opens, as the developer reads it: relative to the project.
pub fn studio_folder_shown() -> String {
    format!("{}/{}", mn::ANDROID_DIR, mn::ANDROID_STUDIO_SUBDIR)
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
    fn a_made_project_is_recorded_in_the_mobile_settings_012() {
        let mut m = MobileSettings::new(cobolt_compiler::project_kind::MobilePlatform::Android, cobolt_compiler::project_kind::DeviceClass::Phone);
        assert!(m.native.is_default());
        record_android(&mut m);
        assert_eq!(m.native.android, "android");
        assert_eq!(studio_folder_shown(), "android/src-tauri/gen/android");
    }
}
