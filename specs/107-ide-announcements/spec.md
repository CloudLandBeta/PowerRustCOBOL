# Spec — IDE announcements ("What's going on")

- **Status:** approved 2026-10-17, from the operator's request. No open questions: each choice the request left open is decided below for the long term (operator's standing instruction), and recorded in §8.
- **Folder:** specs/107-ide-announcements/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-17
- **Classification:** feature — a new IDE capability (a window fed from the repository). It takes the next fix number `z`, never the minor.

## 1. Overview

The operator, 2026-10-17:

> Create a folder in the GitHub project to hold announcements. I'll provide the text, that will appear once for the user with a button to close it (until a new announcement is published in the GitHub). The announcement will be an HTML page, so the announcement window must be 70% of the screen resolution (width/height). The window title will be "What's going on". If no file is available, the window should not appear. The filename in the GitHub will be `announcement.html`.

## 2. What exists today

*Mapped 2026-10-17.*

- **Asking GitHub.** `update_check.rs` asks GitHub about releases:
  - once per start-up, on a background thread, with a 10 s timeout;
  - silent on every failure;
  - the repository is fixed in the code by design (`RELEASES_API`).
- **The repository is public.** `https://raw.githubusercontent.com/CloudLandBeta/PowerRustCOBOL/main/README.md` answers 200, so a file in the repository can be read by every installed IDE, without credentials.
- **HTML.** The product already renders HTML: the **Viewer** control (spec 058), with `Source` = a path or an `https://` address.
  - It renders a safe subset of HTML, styled by the page's own CSS: block and inline layout, typography, colours, borders, tables, flex and grid.
  - No script runs.
  - `Layout = Web` formats the page with no margins; `Fullscreen` hides the toolbar.
  - **Known limits:** links are drawn in link colour but not followed when clicked, and an `<img>` shows its `alt` text instead of the picture.
- **Machine-level preferences** live in `ui.toml` (`UiPrefs`).
- **Window rule.** A window never resizes itself (CONVENTIONS).

## 3. Goals / Non-goals

**Goals:**
- An `announcements/` folder in the repository.
- The IDE shows `announcements/announcement.html`, when there is one, once per published version of it, in a window titled *What's going on*, at 70 % of the screen, with a Close button.

**Non-goals:**
- A history of past announcements.
- Announcements per language or per version of the IDE.
- Turning announcements off.
- A Help menu item to reopen the last one.
- Following links and showing pictures. These are the Viewer's own limits, and belong to the Viewer, where every Viewer control gains them (§7).

## 4. Requirements (EARS)

### The repository

- **R1** The repository shall have an `announcements/` folder. Its `README.md` says what the IDE does with `announcement.html`, so whoever writes one knows.
  - A folder git cannot keep empty needs a file anyway.
  - The README is never shown to the IDE's users.

### Fetching

- **R2** The IDE shall read `https://raw.githubusercontent.com/CloudLandBeta/PowerRustCOBOL/main/announcements/announcement.html`:
  - once at start-up and again every 6 hours while it runs;
  - on a background thread, with a 10 s timeout and the IDE's version as its User-Agent, as the update check does.
- **R3** An announcement exists when, and only when, the answer is HTTP 200 with a body that is not blank. Every other outcome means *no announcement*, and nothing is shown, logged to the developer or retried sooner:
  - a 404 (no file);
  - any other status;
  - a network failure, a timeout or TLS trouble;
  - an empty or whitespace-only body.
- **R4** An announcement's identity shall be a stable 64-bit FNV-1a hash of its bytes. Any edit to the file is a new announcement. The hash must stay the same across IDE builds and Rust versions.

### Showing

- **R5** When an announcement exists and its hash is not the one recorded as dismissed, the IDE shall show it once the screen is free. "Free" means none of these is open: the first-run dialogs (toolchain, Ollama, AI setup), the agent wizard, the Walkthrough, a modal of the IDE.
- **R6** The window:
  - shall be titled **What's going on** (a `Tr` string, six languages);
  - shall be centred;
  - shall be **70 % of the screen's width and height**. *Screen* is the monitor the IDE is on, when the platform reports it, else the IDE's window. Either way it is clamped to 95 % of the IDE's window, because an egui window cannot leave the IDE's own.
  - shall keep its size: it never resizes itself, and has no grip, since the operator fixed the size.
- **R7** The page shall be rendered by the Viewer engine: its HTML parser and block painter, through `cobolt_forms::paint::paint_html_page`, on a white page that scrolls inside the window.
  - The HTML is not run as a browser would run it: no script, nothing fetched.
  - *Decided during implementation (2026-10-17):* not a whole Viewer control hosted in the window. Its toolbar cannot be hidden in a window: `Fullscreen` opens an overlay of its own, and `Streamed` shows a conversation, not a `Source`. Its asynchronous loading would also be a second channel for a page the IDE has already fetched. The engine is the same either way.
- **R8** The window shall have a **Close** button and the window's own ×.
  - Either one records the announcement's hash as dismissed in `ui.toml`, so it is not shown again until the file changes.
  - Quitting the IDE without closing it records nothing, so it is shown at the next start.
- **R9** A check that finds a new announcement while one is open shall replace it. A check that finds none shall leave an open window alone: what the developer is reading does not vanish.

### Fit

- **R10** Every new string shall be a `Tr` field in the six languages.
- **R11** The Developer's Guide (English only, GOLDEN RULE #8) shall say what the window is and where it comes from.
- **R12** Tests shall cover:
  - what counts as an announcement (R3);
  - the hash (R4);
  - when it is shown and when not (R5, R8);
  - the window's size, which is 70 % and does not change over frames (R6).

## 5. Acceptance criteria

- [ ] AC1 — With no `announcement.html` in the repository (today's state, 404), the IDE starts and shows nothing (R3).
- [ ] AC2 — With a file published, the window opens titled *What's going on*, at 70 % of the screen (clamped to the IDE), and stays that size over 120 frames (R5, R6).
- [ ] AC3 — Close, then restart: not shown. Edit the file, restart: shown again (R4, R8).
- [ ] AC4 — Offline: nothing shown, no error (R3).
- [ ] AC5 — The six-language string test passes (R10).

## 6. Constraints & steering check

- **i18n:** yes, six languages.
- **Generated-code contract:** none touched.
- **`.cfrm` / project formats:** none touched; one new `ui.toml` field (`announcement_dismissed`), with a default for older files.
- **Docs:** the Guide.
- **Network:** one GET to GitHub at start and every 6 hours, as the update check already does. No data is sent but the User-Agent.

## 7. Risks and follow-ups

- **Links and pictures** are the Viewer engine's limits today. An announcement can be styled text, tables and CSS boxes; a link reads as a link but does not open, and a picture shows its alt text. The follow-up belongs in the Viewer: follow `http(s)` links in the operating system's browser, and draw `<img>` from a `data:` URI or the document's own folder. Every Viewer control gains them.
- **GitHub's CDN** caches raw files for a few minutes, so a newly published file may take that long to reach the IDE.

## 8. Decisions taken for the long term

- The **hash**, not the file's date or GitHub's ETag: it is the content the developer saw, and it does not depend on GitHub's caching headers.
- **6 hours** between checks: an IDE left open for days still learns of a new announcement, at four requests a day.
- **Dismissal by Close**, not by showing: a window the developer never acknowledged is shown again.
- The **Viewer engine**, not a second HTML renderer: one implementation, whose subset is already documented for the product's users.
