// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The window that offers a newer release, and the reply it waits for.
//!
//! [`crate::update_check`] asks GitHub once per start-up. This is what the IDE
//! keeps of the answer: the receiver the reply arrives on and the release it
//! found, and the window that offers it.
//!
//! **It is drawn by the frame, not by a pane.** It used to be drawn from
//! `show_settings_pane`, so the reply was only collected — and the offer only
//! painted — while Project Settings was the Main Pane. A developer who never
//! opened it never learned that a newer release existed. Whoever owns the frame
//! calls [`UpdateOffer::show`] every frame, whatever the Main Pane shows.
//!
//! Declining is **not** remembered: the offer is forgotten with the run, and the
//! next start-up asks again (operator, 2026-09-14).

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use crate::update_check::ReleaseInfo;

/// How soon the frame looks for the reply again while it is still on its way.
/// The reply arrives on another thread and nothing else wakes an idle IDE for it;
/// the wait is bounded by the check's own time-out, so this is a handful of
/// frames at start-up and none afterwards.
const WAIT: Duration = Duration::from_millis(500);

/// The window's size. Its own, never its content's — a window may not resize
/// itself.
const SIZE: egui::Vec2 = egui::vec2(430.0, 190.0);

/// The start-up release check's answer, and the window that offers it.
#[derive(Default)]
pub struct UpdateOffer {
    /// The check, in flight. Yields exactly one message.
    rx: Option<Receiver<Option<ReleaseInfo>>>,
    /// A newer release to offer. Cleared by either button and never persisted.
    offer: Option<ReleaseInfo>,
}

impl UpdateOffer {
    /// Start the check (once, at start-up), on a thread of its own.
    pub fn start(&mut self, current: &str) {
        self.rx = Some(crate::update_check::spawn_check(current));
    }

    /// Watch a channel the caller feeds (tests).
    #[cfg(test)]
    fn watching(rx: Receiver<Option<ReleaseInfo>>) -> Self {
        Self { rx: Some(rx), offer: None }
    }

    /// Whether a release is waiting to be offered or being offered.
    #[cfg(test)]
    fn has_offer(&self) -> bool {
        self.offer.is_some()
    }

    /// Collect the check's one reply. `None` covers "already current" and every
    /// failure alike, and neither says anything to the developer — see
    /// `update_check`.
    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(found) => {
                self.offer = found;
                self.rx = None;
            }
            Err(TryRecvError::Disconnected) => self.rx = None,
            Err(TryRecvError::Empty) => {}
        }
    }

    /// One frame: collect the reply, and offer the release unless `blocked` — a
    /// first-run dialog or a modal owns the screen. A blocked offer waits; it is
    /// not lost.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        tr: &crate::i18n::Tr,
        theme: &crate::theme::Theme,
        blocked: bool,
    ) {
        self.poll();
        if self.rx.is_some() {
            ctx.request_repaint_after(WAIT);
        }
        if blocked {
            return;
        }
        let Some(rel) = self.offer.clone() else {
            return;
        };
        let mut dismiss = false;

        egui::Window::new(egui::RichText::new(tr.upd_title).size(16.0).strong())
            .id(egui::Id::new("update_offer"))
            .collapsible(false)
            // Fixed, like every other window here: nothing negotiates its own
            // rectangle against its contents.
            .resizable(false)
            .fixed_size(SIZE)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-24.0, -24.0])
            .show(ctx, |ui| {
                ui.label(tr.upd_available.replacen("{}", &rel.version, 1));
                ui.label(
                    egui::RichText::new(
                        tr.upd_running.replacen("{}", crate::version::VERSION, 1),
                    )
                    .color(theme.text_dim),
                );
                if let Some(asset) = &rel.asset_name {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(tr.upd_asset_for_platform.replacen("{}", asset, 1))
                            .color(theme.text_dim)
                            .small(),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // The URL goes in the tooltip, not the label: the developer
                    // is about to be sent to the open internet and gets to see
                    // where first — the same rule the linker download follows.
                    if ui
                        .button(tr.upd_download)
                        .on_hover_text(&rel.page_url)
                        .clicked()
                    {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(rel.page_url.clone()));
                        dismiss = true;
                    }
                    if ui.button(tr.upd_later).clicked() {
                        dismiss = true;
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr.upd_recheck_note)
                        .color(theme.text_dim)
                        .small(),
                );
            });

        if dismiss {
            // Forgotten, not remembered. The next start-up asks again.
            self.offer = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn release() -> ReleaseInfo {
        ReleaseInfo {
            version: "9.9.9".to_owned(),
            name: "PowerRustCOBOL 9.9.9".to_owned(),
            page_url: "https://example.invalid/releases/9.9.9".to_owned(),
            asset_name: Some("PowerRustCOBOL-9.9.9-test.dmg".to_owned()),
        }
    }

    /// An offer that has already been collected, as after the reply.
    fn offering() -> UpdateOffer {
        let (tx, rx) = channel();
        tx.send(Some(release())).unwrap();
        let mut o = UpdateOffer::watching(rx);
        o.poll();
        o
    }

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))
    }

    /// One frame that draws the offer and NOTHING else — no Main Pane, no Project
    /// Settings, no panel of any kind. Returns the text it painted.
    fn frame(
        ctx: &egui::Context,
        offer: &mut UpdateOffer,
        tr: &crate::i18n::Tr,
        blocked: bool,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Rect)> {
        let theme = crate::theme::default_theme();
        let mut out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(screen()), events, ..Default::default() },
            |ui| offer.show(ui.ctx(), tr, theme, blocked),
        );
        out.textures_delta.clear();
        fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match s {
                egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect())),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut texts);
        }
        texts
    }

    /// Frames until a window that has just opened is on screen: egui measures a
    /// new window on its first frame and paints nothing, so what is offered is
    /// seen from the second. Returns the text of the last of them.
    fn settle(
        ctx: &egui::Context,
        offer: &mut UpdateOffer,
        tr: &crate::i18n::Tr,
        blocked: bool,
    ) -> Vec<(String, egui::Rect)> {
        let mut texts = Vec::new();
        for _ in 0..3 {
            texts = frame(ctx, offer, tr, blocked, vec![]);
        }
        texts
    }

    fn open(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.area_rect(egui::Id::new("update_offer"))).is_some()
    }

    fn painted(texts: &[(String, egui::Rect)], tr: &crate::i18n::Tr) -> bool {
        let want = tr.upd_available.replacen("{}", "9.9.9", 1);
        texts.iter().any(|(t, _)| *t == want)
    }

    /// The defect: the offer was painted only by the Project Settings pane. This
    /// frame draws no pane at all, and the offer is on screen.
    #[test]
    fn the_offer_is_painted_on_a_frame_that_shows_no_settings_pane() {
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let mut offer = offering();
        let texts = settle(&ctx, &mut offer, &tr, false);
        assert!(painted(&texts, &tr), "the release is offered: {texts:?}");
        assert!(texts.iter().any(|(t, _)| t == tr.upd_title), "titled");
        assert!(texts.iter().any(|(t, _)| t == tr.upd_download), "with its Download button");
        assert!(open(&ctx));
    }

    /// The reply lands on a thread of its own; the frame collects it — wherever
    /// the developer happens to be — and offers it.
    #[test]
    fn the_reply_is_collected_by_the_frame_and_then_offered() {
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let (tx, rx) = channel();
        let mut offer = UpdateOffer::watching(rx);
        let texts = settle(&ctx, &mut offer, &tr, false);
        assert!(!painted(&texts, &tr) && !open(&ctx), "nothing to offer before the reply");
        assert!(offer.rx.is_some(), "still waiting for it");
        tx.send(Some(release())).unwrap();
        frame(&ctx, &mut offer, &tr, false, vec![]);
        assert!(offer.has_offer(), "the very next frame collected it");
        assert!(offer.rx.is_none(), "the receiver is dropped with the reply");
        let texts = settle(&ctx, &mut offer, &tr, false);
        assert!(painted(&texts, &tr), "and it is offered: {texts:?}");
    }

    /// `None` is "already current" and every failure; none of them is a window.
    #[test]
    fn a_check_that_found_nothing_says_nothing() {
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let (tx, rx) = channel();
        tx.send(None).unwrap();
        let mut offer = UpdateOffer::watching(rx);
        let texts = frame(&ctx, &mut offer, &tr, false, vec![]);
        assert!(!offer.has_offer());
        assert!(texts.is_empty(), "no window: {texts:?}");
        // A check that died without answering is the same.
        let (tx, rx) = channel::<Option<ReleaseInfo>>();
        drop(tx);
        let mut offer = UpdateOffer::watching(rx);
        frame(&ctx, &mut offer, &tr, false, vec![]);
        assert!(offer.rx.is_none() && !offer.has_offer());
    }

    /// A first-run dialog owns the screen: the offer waits, unpainted, and is
    /// shown the frame after — not lost.
    #[test]
    fn a_blocked_offer_waits_and_is_shown_when_the_screen_is_free() {
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let (tx, rx) = channel();
        let mut offer = UpdateOffer::watching(rx);
        tx.send(Some(release())).unwrap();
        let texts = settle(&ctx, &mut offer, &tr, true);
        assert!(!painted(&texts, &tr) && !open(&ctx), "nothing while another dialog owns the screen");
        assert!(offer.has_offer(), "collected all the same");
        let texts = settle(&ctx, &mut offer, &tr, false);
        assert!(painted(&texts, &tr), "shown once the screen is free");
    }

    /// The window keeps one rectangle, in every language, over 120 frames: its
    /// width is its own, it never outgrows its stated size, and once it is up
    /// it does not move or change by a pixel (GOLDEN RULE — a window may never
    /// resize itself).
    #[test]
    fn the_window_keeps_one_rectangle_in_every_language_over_many_frames() {
        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            let ctx = egui::Context::default();
            let mut offer = offering();
            let mut settled = None;
            for n in 0..120 {
                frame(&ctx, &mut offer, &tr, false, vec![]);
                if n < 3 {
                    continue;
                }
                let got = ctx
                    .memory(|m| m.area_rect(egui::Id::new("update_offer")))
                    .expect("shown");
                let first = *settled.get_or_insert(got);
                assert_eq!(got, first, "{lang:?} frame {n}: the window changed on its own");
                assert!(
                    (got.width() - SIZE.x).abs() < 1.0 && got.height() <= SIZE.y + 1.0,
                    "{lang:?} frame {n}: {:?} is not within {SIZE:?}",
                    got.size()
                );
            }
        }
    }

    /// "Later" forgets the offer for this run.
    #[test]
    fn later_dismisses_the_offer() {
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let mut offer = offering();
        frame(&ctx, &mut offer, &tr, false, vec![]);
        let texts = frame(&ctx, &mut offer, &tr, false, vec![]);
        let later = texts.iter().find(|(t, _)| t == tr.upd_later).expect("the Later button").1.center();
        let press = |pressed| egui::Event::PointerButton {
            pos: later,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut offer, &tr, false, vec![egui::Event::PointerMoved(later)]);
        frame(&ctx, &mut offer, &tr, false, vec![press(true)]);
        frame(&ctx, &mut offer, &tr, false, vec![press(false)]);
        assert!(!offer.has_offer(), "Later forgets the offer");
        let texts = frame(&ctx, &mut offer, &tr, false, vec![]);
        assert!(!painted(&texts, &tr), "and the window is gone");
    }
}
