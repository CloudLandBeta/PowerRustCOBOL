// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 107 — *What's going on*: the announcement the repository publishes.
//!
//! The operator writes `announcements/announcement.html` in the repository. The IDE reads
//! it at start-up and every six hours, on a background thread, and shows it ONCE: closing
//! the window records the page's identity (a hash of its bytes) in `ui.toml`, so it is not
//! shown again until the file changes. No file, a network that is not there, a page that is
//! blank: no window, and nothing said — as the update check, an announcement must never be
//! an interruption of its own.
//!
//! The page is painted by the Viewer control's engine (`cobolt_forms::paint::paint_html_page`):
//! its HTML subset styled by the page's own CSS, on a white page, and never run as a browser
//! would run it — no script, nothing fetched.

use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Where the announcement lives: the repository's default branch, read as a raw file. The
/// repository is fixed, as the update check's is — this is *this* product's news.
pub const URL: &str =
    "https://raw.githubusercontent.com/CloudLandBeta/PowerRustCOBOL/main/announcements/announcement.html";

/// Short: the first check runs while the IDE starts (R2).
const TIMEOUT: Duration = Duration::from_secs(10);

/// Between checks while the IDE runs, so an IDE left open for days still learns of a new
/// announcement (R2).
pub const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

/// The window's share of the screen, and its cap on the IDE's own window (R6).
const SCREEN_SHARE: f32 = 0.70;
const IDE_SHARE: f32 = 0.95;
/// The title bar, the margins and the Close row: what the window keeps above and below the
/// page — a constant, so nothing measures the space it was handed.
const CHROME_H: f32 = 77.0;
/// Space between the white page's edge and its text.
const PAGE_PAD: f32 = 18.0;
/// The page's base text size; CSS lengths scale from it (16 CSS px = this).
const PAGE_FONT: f32 = 15.0;

/// One announcement: the page, and its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub html: String,
    /// [`identity`] of the page's bytes.
    pub id: String,
}

/// R3 — what an answer means. Only HTTP 200 with a body that is not blank is an
/// announcement; every other answer means there is none.
pub fn from_response(status: u16, body: &str) -> Option<Announcement> {
    (status == 200 && !body.trim().is_empty()).then(|| Announcement { id: identity(body.as_bytes()), html: body.to_owned() })
}

/// R4 — a page's identity: FNV-1a over its bytes, 64 bits, in hex. Any edit is a new
/// announcement, and the value is the same in every build of the IDE (std's hasher makes
/// no such promise).
pub fn identity(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// R6 — the window's size: 70 % of the screen (the monitor when the platform says how big
/// it is, else the IDE's window), never more than 95 % of the IDE's window, which an egui
/// window cannot leave.
pub fn window_size(monitor: Option<egui::Vec2>, ide: egui::Vec2) -> egui::Vec2 {
    let screen = monitor.filter(|m| m.x > 0.0 && m.y > 0.0).unwrap_or(ide);
    (screen * SCREEN_SHARE).min(ide * IDE_SHARE)
}

/// Start watching: every check sends one message — `Some` for an announcement, `None` for
/// every other outcome — first at once, then every [`RECHECK`]. The thread stops when the
/// receiver is dropped.
pub fn spawn_watch() -> Receiver<Option<Announcement>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || loop {
        if tx.send(fetch()).is_err() {
            return;
        }
        std::thread::sleep(RECHECK);
    });
    rx
}

/// Ask GitHub once. `None` on anything but a page — see [`from_response`].
fn fetch() -> Option<Announcement> {
    // As the update check: ureq with default features off has no TLS until a connector is
    // handed to it.
    let mut builder = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .user_agent(&format!("PowerRustCOBOL/{}", crate::version::VERSION));
    if let Ok(c) = native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(c));
    }
    // ureq answers a 404 as an error: no file, no announcement.
    let resp = builder.build().get(URL).call().ok()?;
    let status = resp.status();
    let body = resp.into_string().ok()?;
    from_response(status, &body)
}

/// The window and what it shows, kept on the app.
#[derive(Default)]
pub struct AnnouncementWindow {
    rx: Option<Receiver<Option<Announcement>>>,
    /// The identity of the page last closed on this machine — read from `ui.toml` once, at
    /// start-up, and kept here, so a frame never reads a file.
    dismissed: String,
    /// The announcement to show (or showing), with its page parsed once.
    current: Option<(Announcement, cobolt_forms::viewer::LayoutDocument)>,
    /// The size the window opened at (R6): fixed while it is open.
    size: Option<egui::Vec2>,
    /// The page's height as last painted, for the scroll area.
    page_h: f32,
}

impl AnnouncementWindow {
    /// Start the checks (once, at start-up), knowing which page was closed last.
    pub fn start(&mut self, dismissed: String) {
        self.dismissed = dismissed;
        self.rx = Some(spawn_watch());
    }

    /// Watch a channel the caller feeds (tests).
    #[cfg(test)]
    fn watching(rx: Receiver<Option<Announcement>>, dismissed: &str) -> Self {
        Self { rx: Some(rx), dismissed: dismissed.to_owned(), ..Default::default() }
    }

    /// Whether an announcement is waiting to be shown or showing.
    pub fn has_announcement(&self) -> bool {
        self.current.is_some()
    }

    /// Read what the checks found since the last frame. A new announcement — not the one
    /// the developer dismissed — replaces whatever was there; a check that found none
    /// leaves an open window alone, so what the developer is reading does not vanish (R9).
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(found) = rx.try_recv() {
            let Some(a) = found else { continue };
            if a.id == self.dismissed || self.current.as_ref().is_some_and(|(c, _)| c.id == a.id) {
                continue;
            }
            let doc = cobolt_forms::viewer::parse_html(&a.html);
            self.current = Some((a, doc));
            self.size = None;
            self.page_h = 0.0;
        }
    }

    /// Show the announcement when there is one and `blocked` is false (another first-run
    /// dialog or modal owns the screen, R5). Returns the identity the developer dismissed
    /// this frame — with Close or the window's × — for the caller to record (R8).
    pub fn show(&mut self, ctx: &egui::Context, tr: &crate::i18n::Tr, blocked: bool) -> Option<String> {
        if blocked {
            return None;
        }
        let (announcement, doc) = self.current.as_ref()?;
        let ide = ctx.content_rect().size();
        let size = *self
            .size
            .get_or_insert_with(|| window_size(ctx.input(|i| i.viewport().monitor_size), ide));
        // Opened at its size; it follows the IDE's window only when that becomes smaller
        // than the window is — never because of what it shows.
        let size = size.min(ide * IDE_SHARE);
        let mut open = true;
        let mut close = false;
        let page_h = &mut self.page_h;
        egui::Window::new(tr.announcement_title)
            .id(egui::Id::new("ide_announcement"))
            .collapsible(false)
            // CONVENTIONS' window rule: the size is the window's own, never its content's.
            .resizable(false)
            .fixed_size(size)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .open(&mut open)
            .show(ctx, |ui| {
                let width = crate::panels::fixed_dialog::content_width(ui, size.x);
                ui.set_width(width);
                let body_h = (size.y - CHROME_H).max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt("ide_announcement_page")
                    .max_height(body_h)
                    .min_scrolled_height(body_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // The page is a document: black on white whatever the IDE's theme,
                        // as the Viewer shows one.
                        let page_w = (width - 14.0).max(80.0);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(page_w, page_h.max(body_h)), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 6.0, egui::Color32::WHITE);
                        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                        let h = cobolt_forms::paint::paint_html_page(
                            &painter,
                            doc,
                            rect.min + egui::vec2(PAGE_PAD, PAGE_PAD),
                            page_w - 2.0 * PAGE_PAD,
                            PAGE_FONT,
                        );
                        *page_h = h + 2.0 * PAGE_PAD;
                    });
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr.announcement_close).clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
            let id = announcement.id.clone();
            self.dismissed = id.clone();
            self.current = None;
            self.size = None;
            return Some(id);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_page_that_is_there_and_not_blank_is_an_announcement() {
        assert!(from_response(200, "<h1>Hello</h1>").is_some());
        for (status, body) in [(404, "Not Found"), (500, "<h1>x</h1>"), (200, ""), (200, "  \n\t ")] {
            assert!(from_response(status, body).is_none(), "{status} {body:?}");
        }
        let a = from_response(200, "<p>Hi</p>").unwrap();
        assert_eq!(a.id, identity(b"<p>Hi</p>"));
    }

    #[test]
    fn identity_is_fnv_1a_and_any_edit_changes_it() {
        // The published FNV-1a 64 test vectors: "" and "a".
        assert_eq!(identity(b""), "cbf29ce484222325");
        assert_eq!(identity(b"a"), "af63dc4c8601ec8c");
        assert_ne!(identity(b"<p>News</p>"), identity(b"<p>News.</p>"));
    }

    #[test]
    fn the_window_is_seventy_percent_of_the_screen_within_the_ide() {
        let ide = egui::vec2(1600.0, 1000.0);
        assert_eq!(window_size(None, ide), egui::vec2(1120.0, 700.0), "no monitor: the IDE's window");
        assert_eq!(window_size(Some(egui::vec2(1920.0, 1080.0)), ide), egui::vec2(1344.0, 756.0), "the monitor");
        // A monitor much bigger than the IDE: capped at 95 % of the IDE.
        assert_eq!(window_size(Some(egui::vec2(5120.0, 2880.0)), ide), egui::vec2(1520.0, 950.0));
        assert_eq!(window_size(Some(egui::vec2(0.0, 0.0)), ide), egui::vec2(1120.0, 700.0), "an unknown monitor");
    }

    #[test]
    fn a_new_page_replaces_the_old_and_a_failed_check_leaves_it_and_a_dismissed_one_stays_away() {
        let (tx, rx) = channel();
        let mut w = AnnouncementWindow::watching(rx, "");
        let a = from_response(200, "<p>One</p>").unwrap();
        let b = from_response(200, "<p>Two</p>").unwrap();
        tx.send(Some(a.clone())).unwrap();
        w.poll();
        assert_eq!(w.current.as_ref().map(|(c, _)| c.id.clone()), Some(a.id.clone()));
        tx.send(None).unwrap();
        w.poll();
        assert!(w.has_announcement(), "a check that found nothing leaves it (R9)");
        tx.send(Some(b.clone())).unwrap();
        w.poll();
        assert_eq!(w.current.as_ref().map(|(c, _)| c.id.clone()), Some(b.id.clone()), "a new page replaces it");
        let mut fresh = AnnouncementWindow::watching(
            {
                let (tx, rx) = channel();
                tx.send(Some(a.clone())).unwrap();
                rx
            },
            &a.id,
        );
        fresh.poll();
        assert!(!fresh.has_announcement(), "the page the developer closed is not shown again");
    }

    /// R6 in every language: the window is its size whatever the title's script.
    #[test]
    fn the_window_is_its_size_in_every_language() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
        let want = window_size(None, screen.size());
        for &lang in crate::i18n::Language::ALL {
            let (tx, rx) = channel();
            tx.send(from_response(200, "<h1>Hi</h1><p>A short page.</p>")).unwrap();
            let mut w = AnnouncementWindow::watching(rx, "");
            w.poll();
            let tr = lang.tr();
            let ctx = egui::Context::default();
            for _ in 0..10 {
                let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                    w.show(ui.ctx(), &tr, false);
                });
                out.textures_delta.clear();
            }
            let got = ctx.memory(|m| m.area_rect(egui::Id::new("ide_announcement"))).expect("shown").size();
            assert!((got.x - want.x).abs() < 1.0 && (got.y - want.y).abs() < 1.0, "{lang:?}: {got:?}, not {want:?}");
        }
    }

    /// AC2/AC3 through real frames: titled, 70 % of the screen and still that size after 120
    /// frames; nothing while blocked; Close dismisses it and says which page.
    #[test]
    fn the_window_holds_its_size_and_close_dismisses_the_page() {
        let (tx, rx) = channel();
        let mut w = AnnouncementWindow::watching(rx, "");
        let a = from_response(
            200,
            "<html><body><h1>What's new</h1><p>PowerRustCOBOL 1.92 is out.</p><ul><li>One</li><li>Two</li></ul></body></html>",
        )
        .unwrap();
        tx.send(Some(a.clone())).unwrap();
        w.poll();
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let mut frame = |w: &mut AnnouncementWindow, blocked: bool, events: Vec<egui::Event>| {
            let mut dismissed = None;
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| {
                dismissed = w.show(ui.ctx(), &tr, blocked);
            });
            out.textures_delta.clear();
            let mut texts = Vec::new();
            fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
                match s {
                    egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect())),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    _ => {}
                }
            }
            for cs in &out.shapes {
                walk(&cs.shape, &mut texts);
            }
            (dismissed, texts)
        };
        let (_, texts) = frame(&mut w, true, vec![]);
        assert!(!texts.iter().any(|(t, _)| t == tr.announcement_title), "nothing while another dialog owns the screen");
        let mut sizes = Vec::new();
        let mut last = Vec::new();
        for _ in 0..120 {
            let (d, texts) = frame(&mut w, false, vec![]);
            assert!(d.is_none());
            sizes.push(ctx.memory(|m| m.area_rect(egui::Id::new("ide_announcement"))).expect("the window is shown").size());
            last = texts;
        }
        assert!(last.iter().any(|(t, _)| t == tr.announcement_title), "titled What's going on");
        assert!(last.iter().any(|(t, _)| t == "What's new"), "the page's heading is painted");
        let want = window_size(None, screen.size());
        for s in &sizes {
            assert!((s.x - want.x).abs() < 1.0 && (s.y - want.y).abs() < 1.0, "the window is {s:?}, not {want:?}");
        }
        let close = last.iter().find(|(t, _)| t == tr.announcement_close).expect("the Close button").1.center();
        let press = |pressed| egui::Event::PointerButton {
            pos: close,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut w, false, vec![egui::Event::PointerMoved(close)]);
        frame(&mut w, false, vec![press(true)]);
        let (d, _) = frame(&mut w, false, vec![press(false)]);
        assert_eq!(d, Some(a.id.clone()), "Close dismisses this page");
        assert!(!w.has_announcement());
        println!("announcement window {:?} at 1600×1000 (70 %), held over 120 frames; Close dismissed {}", want, a.id);
    }
}
