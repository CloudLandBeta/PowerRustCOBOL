// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The window of Package Project (spec 115): the system the package is for, and the language of its install script.
//!
//! It opens before the folder dialog, and only when there is something to choose: another system's runner is on this
//! installation's list, or the project already remembers a system. Otherwise Package Project is what it was in spec 113:
//! the folder dialog and the package of this computer's own system.
//!
//! One question can follow. When the runner of the chosen system is not on this computer, the window asks, before
//! anything is downloaded, whether it may fetch it from the release's attachments (spec 115 D5): nothing is downloaded
//! unless the developer says yes. The state is a plain struct with no painting in it, so each step is a test.
//!
//! A window never resizes itself: it has one width of its own ([`W`]) and [`crate::panels::fixed_dialog`] lays it out.

use cobolt_compiler::backend_runners::System;

use crate::i18n::{Language, Tr};
use crate::web_package::Choice;

/// The window's outer width.
pub const W: f32 = 520.0;

/// Where the window is: choosing, or asking whether the runner of a system may be downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Choose,
    Ask(System),
}

/// What the developer answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Closed, or cancelled: nothing is packaged and nothing is remembered.
    Cancel,
    /// What to package for; the folder dialog follows.
    Go(Choice),
}

/// The window of Package Project.
#[derive(Debug, Clone)]
pub struct PackageModal {
    /// The systems this installation can package for, this computer's own first.
    offered: Vec<System>,
    /// The chosen system; `None` is this computer's own.
    system: Option<System>,
    /// The language of the install script.
    lang: Language,
    stage: Stage,
}

impl PackageModal {
    /// The window for a project that remembers `remembered` (`[web] package_system`, empty for this computer's own).
    /// A remembered system this installation no longer offers is not selected: the window starts on this computer.
    pub fn new(offered: &[System], remembered: &str, lang: Language) -> Self {
        let host = System::host();
        let system = System::parse(remembered).filter(|s| offered.contains(s) && Some(*s) != host);
        Self { offered: offered.to_vec(), system, lang, stage: Stage::Choose }
    }

    /// The chosen system; `None` is this computer's own.
    pub fn system(&self) -> Option<System> {
        self.system
    }

    /// The language of the install script.
    pub fn lang(&self) -> Language {
        self.lang
    }

    /// The system whose runner the window is asking to download, while it asks.
    pub fn asking(&self) -> Option<System> {
        match self.stage {
            Stage::Ask(s) => Some(s),
            Stage::Choose => None,
        }
    }

    /// The systems other than this computer's own, as the combo lists them after "this computer".
    pub fn others(&self) -> impl Iterator<Item = System> + '_ {
        let host = System::host();
        self.offered.iter().copied().filter(move |s| Some(*s) != host)
    }

    /// Choose a system; `None` is this computer's own. A system nobody offers is not taken.
    pub fn select(&mut self, system: Option<System>) {
        if system.is_none_or(|s| self.offered.contains(&s)) {
            self.system = system.filter(|s| Some(*s) != System::host());
        }
    }

    pub fn set_lang(&mut self, lang: Language) {
        self.lang = lang;
    }

    fn choice(&self, fetch_runner: bool) -> Choice {
        Choice { system: self.system, runner_dir: None, fetch_runner, lang: Some(self.lang.code()) }
    }

    /// *Choose the folder…*. `needs_download` says whether packaging for the chosen system would have to download its runner
    /// (`cobolt_project_tools::web::package::needs_runner_download`): then the window asks first and answers nothing yet.
    pub fn proceed(&mut self, needs_download: impl FnOnce(Option<System>) -> bool) -> Option<Answer> {
        if self.stage != Stage::Choose {
            return None;
        }
        match self.system {
            Some(system) if needs_download(self.system) => {
                self.stage = Stage::Ask(system);
                None
            }
            _ => Some(Answer::Go(self.choice(false))),
        }
    }

    /// The answer to the question before a download. Yes lets the package download the runner (and say so); no downloads
    /// nothing and takes the developer back to the choice, where another system or Cancel is one click away.
    pub fn answer_download(&mut self, yes: bool) -> Option<Answer> {
        if self.asking().is_none() {
            return None;
        }
        if yes {
            return Some(Answer::Go(self.choice(true)));
        }
        self.stage = Stage::Choose;
        None
    }

    /// Draw the window and return what the developer answered, once. `needs_download` is asked only when *Choose the
    /// folder…* is pressed with another system chosen.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Tr, needs_download: impl FnOnce(Option<System>) -> bool) -> Option<Answer> {
        let mut open = true;
        let mut answer = None;
        crate::panels::fixed_dialog::show(ctx, tr.pkgsys_title, W, &mut open, |ui| match self.stage {
            Stage::Choose => {
                ui.add(egui::Label::new(tr.pkgsys_intro).wrap());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(tr.pkgsys_system_label);
                    let shown = self.system.map_or_else(|| tr.pkgsys_this_computer.to_owned(), |s| s.to_string());
                    egui::ComboBox::from_id_salt("package_modal_system").selected_text(shown).show_ui(ui, |ui| {
                        if ui.selectable_label(self.system.is_none(), tr.pkgsys_this_computer).clicked() {
                            self.select(None);
                        }
                        let others: Vec<System> = self.others().collect();
                        for s in others {
                            if ui.selectable_label(self.system == Some(s), s.to_string()).clicked() {
                                self.select(Some(s));
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(tr.pkgsys_lang_label);
                    egui::ComboBox::from_id_salt("package_modal_lang").selected_text(self.lang.native_name()).show_ui(ui, |ui| {
                        for &l in Language::ALL {
                            if ui.selectable_label(self.lang == l, l.native_name()).clicked() {
                                self.set_lang(l);
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.pkgsys_continue).clicked() {
                        answer = self.proceed(needs_download);
                    }
                    if ui.button(tr.btn_cancel).clicked() {
                        answer = Some(Answer::Cancel);
                    }
                });
            }
            Stage::Ask(system) => {
                ui.add(egui::Label::new(tr.pkgsys_fetch_question.replace("{system}", &system.to_string())).wrap());
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.pkgsys_fetch_yes).clicked() {
                        answer = self.answer_download(true);
                    }
                    if ui.button(tr.pkgsys_fetch_no).clicked() {
                        answer = self.answer_download(false);
                    }
                });
            }
        });
        if !open {
            return Some(Answer::Cancel);
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::backend_runners::{Arch, Os};
    use std::time::Instant;

    fn sys(word: &str) -> System {
        System::parse(word).unwrap()
    }

    /// A machine that is not this one's host for sure: the window's logic does not depend on which machine runs the test,
    /// so the lists below are built with the host in front, as `System::offered` does.
    fn offered_with(extra: &[System]) -> Vec<System> {
        let mut out: Vec<System> = System::host().into_iter().collect();
        for s in extra {
            if !out.contains(s) {
                out.push(*s);
            }
        }
        out
    }

    fn other() -> System {
        let linux = System { os: Os::Linux, arch: Arch::X86_64 };
        if System::host() == Some(linux) {
            System { os: Os::Linux, arch: Arch::Aarch64 }
        } else {
            linux
        }
    }

    /// Spec 115 T14 (R1, R8, D5, D11): the window starts on the remembered system, hands back what was chosen, asks about a
    /// download only when one would happen and takes no for an answer, and a Desktop-style host-only list is a plain choice.
    #[test]
    fn the_window_starts_where_the_project_left_it_and_asks_before_a_download_only() {
        let began = Instant::now();
        let other = other();
        let offered = offered_with(&[other]);
        let mut rows: Vec<(&str, bool)> = Vec::new();
        let mut check = |what: &'static str, ok: bool| {
            rows.push((what, ok));
            assert!(ok, "{what}");
        };

        let fresh = PackageModal::new(&offered, "", Language::Portuguese);
        check("nothing remembered: this computer's own system", fresh.system().is_none());
        check("the script starts in the IDE's language", fresh.lang() == Language::Portuguese);
        check("the others are listed after this computer", fresh.others().collect::<Vec<_>>() == vec![other]);

        let remembered = PackageModal::new(&offered, &other.to_string(), Language::English);
        check("a remembered system that is offered is the one chosen", remembered.system() == Some(other));
        let host_word = System::host().map(|h| h.to_string()).unwrap_or_default();
        check("the word of this computer's own system is this computer", PackageModal::new(&offered, &host_word, Language::English).system().is_none());
        check("a remembered system nobody offers is not chosen", PackageModal::new(&offered_with(&[]), &other.to_string(), Language::English).system().is_none());
        check("a remembered word that is no system is not chosen", PackageModal::new(&offered, "plan9-m68k", Language::English).system().is_none());

        // Choosing.
        let mut m = PackageModal::new(&offered, "", Language::English);
        let nobody = ["windows-x86_64", "windows-aarch64", "macos-x86_64", "macos-aarch64", "linux-x86_64", "linux-aarch64"].into_iter().map(sys).find(|s| !offered.contains(s)).expect("six systems, two offered");
        m.select(Some(nobody));
        check("a system nobody offers is not taken", m.system().is_none());
        m.select(Some(other));
        check("an offered system is taken", m.system() == Some(other));
        m.select(None);
        check("this computer is taken back", m.system().is_none());
        m.set_lang(Language::French);

        // This computer's own system never asks, whatever the answer to the question would be.
        let mut asked = false;
        let answer = m.proceed(|_| {
            asked = true;
            true
        });
        check("this computer's own system never asks about a download", !asked);
        check("and answers with no system, no download and the chosen script language", answer == Some(Answer::Go(Choice { system: None, runner_dir: None, fetch_runner: false, lang: Some("fr") })));

        // Another system whose runner is here: no question.
        m.select(Some(other));
        let answer = m.proceed(|s| s != Some(other));
        check("another system whose runner is here goes straight on", answer == Some(Answer::Go(Choice { system: Some(other), runner_dir: None, fetch_runner: false, lang: Some("fr") })));

        // Another system whose runner is not here: the question, and both answers.
        let mut m = PackageModal::new(&offered, &other.to_string(), Language::Spanish);
        check("nothing is asked before Choose the folder is pressed", m.asking().is_none());
        check("a runner that is not here asks and answers nothing yet", m.proceed(|_| true).is_none() && m.asking() == Some(other));
        check("while it asks, Choose the folder does nothing", m.proceed(|_| false).is_none() && m.asking() == Some(other));
        check("no downloads nothing and goes back to the choice", m.answer_download(false).is_none() && m.asking().is_none() && m.system() == Some(other));
        check("a no with nothing asked is nothing", m.answer_download(true).is_none());
        let _ = m.proceed(|_| true);
        let yes = m.answer_download(true);
        check("yes lets the package download, for that system, in the chosen language", yes == Some(Answer::Go(Choice { system: Some(other), runner_dir: None, fetch_runner: true, lang: Some("es") })));

        // The list of this computer alone is a plain choice with no other line.
        let alone = PackageModal::new(&offered_with(&[]), "", Language::English);
        check("only this computer offered: no other line", alone.others().next().is_none());

        println!("\n== Spec 115 T14: the window of Package Project, without painting ==");
        for (what, ok) in &rows {
            println!("  {}   {what}", if *ok { "ok  " } else { "FAIL" });
        }
        println!("  {} cases, {} passed, {:.1} ms", rows.len(), rows.iter().filter(|(_, ok)| *ok).count(), began.elapsed().as_secs_f64() * 1000.0);
    }

    /// The window keeps one rectangle over 120 frames at each of its two stages in every language, at the width it owns
    /// ([`W`]), and answers nothing while nobody clicks: it does not resize itself (CONVENTIONS.md).
    #[test]
    fn the_window_keeps_its_rectangle_in_six_languages_and_at_both_stages() {
        let began = Instant::now();
        let other = other();
        let offered = offered_with(&[other]);
        let mut frames_checked = 0;
        let mut widths = std::collections::BTreeSet::new();
        for &lang in Language::ALL {
            let tr = lang.tr();
            for stage in 0..2 {
                let mut m = PackageModal::new(&offered, &other.to_string(), lang);
                if stage == 1 {
                    let _ = m.proceed(|_| true);
                    assert_eq!(m.asking(), Some(other), "{lang:?}");
                }
                let ctx = egui::Context::default();
                crate::app::apply_glass_visuals(&ctx, crate::theme::default_theme());
                let mut first: Option<egui::Rect> = None;
                for frame in 0..120 {
                    let mut input = egui::RawInput::default();
                    input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0)));
                    let mut out = ctx.run_ui(input, |ui| {
                        assert_eq!(m.show(ui.ctx(), &tr, |_| true), None, "{lang:?}: nobody clicked");
                    });
                    out.textures_delta.clear();
                    let rect = ctx.memory(|mem| mem.layer_ids().filter(|l| l.order == egui::Order::Middle).find_map(|l| mem.area_rect(l.id)));
                    let Some(rect) = rect else { continue };
                    if frame >= 2 {
                        match first {
                            None => first = Some(rect),
                            Some(f) => assert_eq!(rect, f, "{lang:?}, stage {stage}: frame {frame} moved or resized the window"),
                        }
                        frames_checked += 1;
                    }
                }
                let f = first.expect("the window was drawn");
                assert!((f.width() - W).abs() < 2.0, "{lang:?}, stage {stage}: {:?}", f.size());
                widths.insert((f.width() * 10.0) as i32);
            }
        }
        println!("\n== Spec 115 T14: the window's rectangle ==\n  {frames_checked} frames over 6 languages x 2 stages, {} distinct width(s), {:.1} ms", widths.len(), began.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(widths.len(), 1, "one width for every language and stage: {widths:?}");
    }
}
