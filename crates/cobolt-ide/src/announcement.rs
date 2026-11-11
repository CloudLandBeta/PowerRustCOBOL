// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 107 — *What's going on*: the announcement the repository publishes.
//!
//! The operator writes `announcements/announcement.html` (English) in the repository, and
//! beside it one page per other language, `announcement.<code>.html` (`es`, `pt`, `ja`, `zh`,
//! `fr`). The IDE reads the page of its own language at start-up and every six hours, on a
//! background thread — the English page when its language has none — and shows it ONCE:
//! closing the window records the page's identity (a hash of its bytes) in `ui.toml`, so it
//! is not shown again until the file changes. No file, a network that is not there, a page
//! that is blank: no window, and nothing said — as the update check, an announcement must
//! never be an interruption of its own.
//!
//! The page is painted by the Viewer control's engine (`cobolt_forms::paint::paint_html_page`):
//! its HTML subset styled by the page's own CSS, on a white page, and never run as a browser
//! would run it — no script, nothing fetched.

use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where the English announcement lives: the repository's default branch, read as a raw file.
/// The repository is fixed, as the update check's is — this is *this* product's news. English
/// keeps the name it always had, so an IDE older than the language pages still reads it.
pub const URL: &str =
    "https://raw.githubusercontent.com/CloudLandBeta/PowerRustCOBOL/main/announcements/announcement.html";

/// Where an announcement lives in `code`'s language (`Language::code`): English is [`URL`],
/// every other language has its own page beside it, `announcement.<code>.html`.
pub fn url_for(code: &str) -> String {
    match code {
        "" | "en" => URL.to_owned(),
        _ => URL.replace("announcement.html", &format!("announcement.{code}.html")),
    }
}

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

/// The IDE's language as the watch thread reads it at every check: the code of
/// `Language::code`, shared so a language chosen while the IDE runs reaches the next check.
type SharedLanguage = Arc<Mutex<&'static str>>;

/// Start watching: every check sends one message — `Some` for an announcement, `None` for
/// every other outcome — first at once, then every [`RECHECK`], each in the language held
/// in `lang` at that moment. The thread stops when the receiver is dropped.
fn spawn_watch(lang: SharedLanguage) -> Receiver<Option<Announcement>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || loop {
        let code = *lang.lock().unwrap_or_else(|e| e.into_inner());
        if tx.send(fetch(code)).is_err() {
            return;
        }
        std::thread::sleep(RECHECK);
    });
    rx
}

/// Ask GitHub once, for the page of `code`'s language. `None` on anything but a page — see
/// [`from_response`].
fn fetch(code: &str) -> Option<Announcement> {
    // As the update check: ureq with default features off has no TLS until a connector is
    // handed to it.
    let mut builder = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .user_agent(&format!("PowerRustCOBOL/{}", crate::version::VERSION));
    if let Ok(c) = native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(c));
    }
    let agent = builder.build();
    fetch_with(code, |url| {
        // ureq answers a 404 as an error: no file, no announcement.
        let resp = agent.get(url).call().ok()?;
        let status = resp.status();
        let body = resp.into_string().ok()?;
        from_response(status, &body)
    })
}

/// The language's page, or the English one when that language has none yet: a language the
/// operator has not written for still hears the news. `get` answers one address.
fn fetch_with(code: &str, mut get: impl FnMut(&str) -> Option<Announcement>) -> Option<Announcement> {
    if !matches!(code, "" | "en") {
        if let Some(page) = get(&url_for(code)) {
            return Some(page);
        }
    }
    get(URL)
}

/// The window and what it shows, kept on the app.
#[derive(Default)]
pub struct AnnouncementWindow {
    rx: Option<Receiver<Option<Announcement>>>,
    /// The IDE's language, which the next check reads.
    lang: SharedLanguage,
    /// The identity of the page last closed on this machine — read from `ui.toml` once, at
    /// start-up, and kept here, so a frame never reads a file.
    dismissed: String,
    /// The announcement to show (or showing), with its page parsed once.
    current: Option<(Announcement, cobolt_forms::viewer::LayoutDocument)>,
    /// The size the window opened at (R6): fixed while it is open.
    size: Option<egui::Vec2>,
    /// The page's height as last painted, for the scroll area.
    page_h: f32,
    /// The answer to the page asked for from Help ▸ What's new (R19), while it is awaited.
    asked: Option<Receiver<Option<Announcement>>>,
    /// The window on screen was asked for, so it does not wait for the screen to be free.
    wanted: bool,
    /// The page asked for could not be read, and nobody has been told yet.
    unavailable: bool,
}

impl AnnouncementWindow {
    /// Start the checks (once, at start-up), knowing which page was closed last and the
    /// language the IDE starts in (`Language::code`).
    pub fn start(&mut self, dismissed: String, code: &'static str) {
        self.dismissed = dismissed;
        self.set_language(code);
        self.rx = Some(spawn_watch(self.lang.clone()));
    }

    /// The language the IDE is in now: the next check asks for that page. A page already
    /// open stays as it is.
    pub fn set_language(&self, code: &'static str) {
        *self.lang.lock().unwrap_or_else(|e| e.into_inner()) = code;
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

    /// R19 — Help ▸ What's new: read the page of the IDE's language now, on a thread of its
    /// own, to show it whether or not it was closed before. The answer reaches [`poll`](Self::poll).
    pub fn ask(&mut self, ctx: &egui::Context) {
        let code = *self.lang.lock().unwrap_or_else(|e| e.into_inner());
        let (tx, rx) = channel();
        // An idle IDE draws no frame of its own: the answer wakes it.
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch(code));
            ctx.request_repaint();
        });
        self.asked = Some(rx);
    }

    /// R21 — whether the page asked for could not be read (no network, none published) since
    /// the last time this was asked. Said once, so the caller can tell the developer once.
    pub fn take_unavailable(&mut self) -> bool {
        std::mem::take(&mut self.unavailable)
    }

    /// Put `a` on screen in place of whatever was there; `wanted` says the developer asked for it.
    fn open(&mut self, a: Announcement, wanted: bool) {
        let doc = cobolt_forms::viewer::parse_html(&a.html);
        self.current = Some((a, doc));
        self.size = None;
        self.page_h = 0.0;
        self.wanted = wanted;
    }

    /// Read what the checks found since the last frame. A new announcement — not the one
    /// the developer dismissed — replaces whatever was there; a check that found none
    /// leaves an open window alone, so what the developer is reading does not vanish (R9).
    /// The page asked for from Help is shown dismissed or not (R20).
    pub fn poll(&mut self) {
        if let Some(rx) = &self.asked {
            match rx.try_recv() {
                Ok(found) => {
                    self.asked = None;
                    match found {
                        Some(a) => self.open(a, true),
                        None => self.unavailable = true,
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.asked = None,
            }
        }
        let Some(rx) = &self.rx else { return };
        let found: Vec<Announcement> = rx.try_iter().flatten().collect();
        for a in found {
            if a.id == self.dismissed || self.current.as_ref().is_some_and(|(c, _)| c.id == a.id) {
                continue;
            }
            self.open(a, false);
        }
    }

    /// Show the announcement when there is one and `blocked` is false (another first-run
    /// dialog or modal owns the screen, R5) — unless the developer asked for it, which waits
    /// for nothing. Returns the identity the developer dismissed this frame — with Close or
    /// the window's × — for the caller to record (R8).
    pub fn show(&mut self, ctx: &egui::Context, tr: &crate::i18n::Tr, blocked: bool) -> Option<String> {
        if blocked && !self.wanted {
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
            self.wanted = false;
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
    fn each_language_reads_its_own_page_and_english_keeps_its_old_name() {
        assert_eq!(url_for("en"), URL, "English is the page every IDE has always read");
        assert_eq!(url_for(""), URL, "no language yet is English");
        let mut seen = std::collections::BTreeSet::new();
        for &lang in crate::i18n::Language::ALL {
            assert!(seen.insert(url_for(lang.code())), "{lang:?} has an address of its own");
        }
        assert_eq!(url_for("pt"), URL.replace("announcement.html", "announcement.pt.html"));
        assert!(url_for("ja").ends_with("/announcements/announcement.ja.html"));
        let w = AnnouncementWindow::default();
        w.set_language("zh");
        assert_eq!(*w.lang.lock().unwrap(), "zh", "the next check reads the language chosen last");
    }

    #[test]
    fn a_language_without_a_page_hears_the_english_one() {
        let en = from_response(200, "<p>English</p>").unwrap();
        let pt = from_response(200, "<p>Portugues</p>").unwrap();
        let pages = |url: &str| match url {
            u if u == URL => Some(en.clone()),
            u if u == url_for("pt") => Some(pt.clone()),
            _ => None,
        };
        let asked = std::cell::RefCell::new(Vec::new());
        let ask = |code: &str| {
            asked.borrow_mut().clear();
            fetch_with(code, |url| {
                asked.borrow_mut().push(url.to_owned());
                pages(url)
            })
        };
        assert_eq!(ask("pt"), Some(pt.clone()), "its own page");
        assert_eq!(*asked.borrow(), vec![url_for("pt")], "one request when its page is there");
        assert_eq!(ask("fr"), Some(en.clone()), "no French page: the English one");
        assert_eq!(*asked.borrow(), vec![url_for("fr"), URL.to_owned()]);
        assert_eq!(ask("en"), Some(en.clone()));
        assert_eq!(*asked.borrow(), vec![URL.to_owned()], "English asks for nothing else");
        assert_eq!(fetch_with("ja", |_| None), None, "no page at all: no announcement");
    }

    /// Whether the window is on screen after a few frames, the screen being free or not.
    fn on_screen(w: &mut AnnouncementWindow, blocked: bool) -> bool {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
        let tr = crate::i18n::Language::English.tr();
        let ctx = egui::Context::default();
        for _ in 0..5 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                w.show(ui.ctx(), &tr, blocked);
            });
            out.textures_delta.clear();
        }
        ctx.memory(|m| m.area_rect(egui::Id::new("ide_announcement"))).is_some()
    }

    /// R19–R21: the page asked for from Help ▸ What's new opens though the developer closed it
    /// before, waits for no free screen (the automatic one does), and a page that cannot be read
    /// is said once and opens nothing.
    #[test]
    fn the_page_asked_for_from_help_opens_though_it_was_closed_and_waits_for_nothing() {
        let a = from_response(200, "<p>Closed before</p>").unwrap();

        // Automatic: a page already closed is not shown again, and one not yet seen waits.
        let mut closed = AnnouncementWindow::watching(channel().1, &a.id);
        let (tx, rx) = channel();
        closed.rx = Some(rx);
        tx.send(Some(a.clone())).unwrap();
        closed.poll();
        assert!(!closed.has_announcement(), "the automatic check leaves a closed page alone");
        let mut fresh = AnnouncementWindow::watching(channel().1, "");
        let (tx, rx) = channel();
        fresh.rx = Some(rx);
        tx.send(Some(a.clone())).unwrap();
        fresh.poll();
        assert!(fresh.has_announcement() && !on_screen(&mut fresh, true), "an unseen page waits for a free screen");

        // Asked for: the same closed page opens, and on a screen that is not free.
        let (tx, rx) = channel();
        closed.asked = Some(rx);
        tx.send(Some(a.clone())).unwrap();
        closed.poll();
        assert!(closed.has_announcement(), "a closed page opens when asked for");
        assert!(on_screen(&mut closed, true), "…and does not wait for a free screen");

        // Asked for, and nothing to read: nothing opens, and it is said once.
        let mut none = AnnouncementWindow::watching(channel().1, "");
        let (tx, rx) = channel();
        none.asked = Some(rx);
        tx.send(None).unwrap();
        none.poll();
        assert!(!none.has_announcement());
        assert!(none.take_unavailable(), "the developer is told");
        assert!(!none.take_unavailable(), "once");
        println!("What's new: a closed page opens when asked for, on a busy screen; an unreadable one is said once");
    }

    /// The text between each `open` and the next `close` in `s`, in order.
    fn between<'a>(s: &'a str, open: &str, close: &str) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut rest = s;
        while let Some(a) = rest.find(open) {
            let after = &rest[a + open.len()..];
            let Some(b) = after.find(close) else { break };
            out.push(&after[..b]);
            rest = &after[b + close.len()..];
        }
        out
    }

    fn page(code: &str) -> String {
        let name = if code == "en" { "announcement.html".to_owned() } else { format!("announcement.{code}.html") };
        std::fs::read_to_string(format!("{}/../../announcements/{name}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|e| panic!("announcements/{name}: {e}"))
    }

    /// The pages the operator publishes: one per language, made like the English one. The
    /// same style sheet (so the same layout), the same structure, and every `<code>` — COBOL,
    /// commands, paths — word for word, because those stay English in every language.
    #[test]
    fn every_language_has_a_page_made_like_the_english_one() {
        let en = page("en");
        assert!(en.contains("<html lang=\"en\">"));
        let count = |s: &str, tag: &str| s.matches(tag).count();
        let tags = ["<h1>", "<h2>", "<h3>", "<ol>", "<li>", "<table>", "<tr>", "<th>", "<td>", "class=\"card\"", "class=\"note\"", "class=\"small\""];
        let mut ids = std::collections::BTreeSet::new();
        ids.insert(from_response(200, &en).unwrap().id);
        for &lang in crate::i18n::Language::ALL.iter().filter(|l| l.code() != "en") {
            let code = lang.code();
            let p = page(code);
            assert!(p.contains(&format!("<html lang=\"{code}\">")), "{code}: lang attribute");
            assert_eq!(between(&p, "<style>", "</style>"), between(&en, "<style>", "</style>"), "{code}: the same style sheet");
            assert_eq!(between(&p, "<code>", "</code>"), between(&en, "<code>", "</code>"), "{code}: COBOL, commands and paths stay English");
            for tag in tags {
                assert_eq!(count(&p, tag), count(&en, tag), "{code}: the number of {tag}");
            }
            assert_ne!(between(&p, "<h1>", "</h1>"), between(&en, "<h1>", "</h1>"), "{code}: the title is translated");
            assert_ne!(between(&p, "<h2>", "</h2>"), between(&en, "<h2>", "</h2>"), "{code}: the headings are translated");
            let a = from_response(200, &p).unwrap_or_else(|| panic!("{code}: not a page"));
            assert!(ids.insert(a.id), "{code}: its own identity, not another language's page");
        }
        println!("announcement pages: {} languages, {} <code> items each, the style sheet and {} structural tags equal to the English page", ids.len(), between(&en, "<code>", "</code>").len(), tags.len());
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
