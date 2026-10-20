<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The WebLink control

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-006-weblink-control/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **6** — a new *WebLink* control in the toolbox: basically a Label that shows a friendly text (the caption) for a URL; the mouse cursor becomes a hand over
  it; properties define the URL and whether it opens in a new tab or a new window; modifier keys (Shift, Control, Command…) open a new tab or window; **every capability of the HTML `<a>` tag**
  is available.
- **Depends on:** WASM-004 (the control model on the web), WASM-007 (its states are styled by CSS). **Precedent:** spec 049 made the `SideMenu` its own control type rather than a MenuBar mode;
  this spec does the same for a link rather than a Label mode.

## 1. Overview

A form often has to point somewhere: the terms of service, a help page, a mail address, a file to download. Today the developer paints a Label and handles the click, and there is no way to
open a URL from COBOL at all. **WebLink** is the control for it: it looks like link text, behaves like an `<a>` element in a browser — including the browser's own conventions for modifier keys and
middle clicks — and exposes the same capabilities through properties.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **Label** (the base): properties Caption, TextAlignment, VerticalAlignment, WordWrap, AutoSize, CornerRadius, BorderStyle (default None) plus the universal set (`cobolt-forms/src/model.rs:5350-5377`,
  layout seeds `:5132`, perspective props from `has_perspective` `:4181`); 22 events (mouse, hover, geometry, lifecycle; no `onChange`) (`:3437`); methods `SetCaption` and `GetCaption` only
  (`cobolt-compiler/src/lib.rs:7617`, `:7685`; `interpreter.rs:16243-16252`). At run time its caption is selectable text (`render.rs:15427`).
- **Cursor** exists on every control: seeded at `model.rs:5273`, mapped at `render.rs:7593`, applied at `render.rs:8096`; the documented domain is
  `Default|Hand|Text|Wait|Crosshair|No|SizeAll|SizeNS|SizeWE` (`cobolt-compiler/src/lib.rs:6285`). The hand cursor needs no new mechanism.
- **Nothing opens a URL.** There is no hyperlink or URL property on any control and no `open_url` in `cobolt-forms`, `cobolt-form-host` or `cobolt-runtime`; the only call is the IDE's own egui one
  (`cobolt-ide/src/app.rs:13318`). The Viewer paints `<a href>` as a link (`viewer.rs:2993`, `paint.rs:10819`) but nothing follows it. COBOL has no `CALL` that opens a URL or starts a process.
- **Adding a control type touches** (counted for the planners): `model.rs` (the enum `:2746`, `ALL` `:2926`, `as_str`, `from_str`, `default_size` `:3140`, `primary_event` `:3207`, `supported_events` `:3371`,
  `is_non_visual` `:4152`, the `Control::new` arm, `runtime_property_names_for` `:1527`); `paint.rs:2957` and `render.rs`; **`cobolt-ast/src/methods.rs:164`** (an unlisted method name parses
  silently as a subscript); the interpreter's method arms; the form host's seeding; `runtime_features.rs:245`; the compiler's KB tables (`control_structure` `:7522`, `control_purpose` `:7534`,
  `control_method_docs` `:7585`, `control_usage_notes` `:8137`, `property_reference_for`, `event_reference`); the toolbox (`TOOLS`, icon `:1954`); `properties.rs`; `designer.rs:17579`; `agent.rs:4850`;
  `prop_help_data.rs`; `ide_text_data.rs`; `i18n.rs`. More than 20 test files iterate `ControlType::ALL`, so a new variant is exercised automatically; `tests/controls/` is hand-kept.

## 2. Goals / Non-goals

- **Goals:**
  - A **WebLink** control in the toolbox for the web and mobile kinds that shows link text and navigates like an `<a>` element.
  - Every capability of `<a>` available as a property, method or event — **except `ping`**, left out of v1 (R3) — and the browser's conventions for modifier keys honoured.
  - Safe by default: dangerous schemes refused, new contexts opened without handing the new page a handle back to this one.
- **Non-goals:**
  - A rich-text editor or inline links inside a Label's text; one WebLink is one link.
  - Navigating *between forms* of the application (that is the application shell, spec 049, and the router proposed in WASM-INDEX).
  - Link previews, link checking or analytics.

## 3. User stories

- As a developer, I want to drop a WebLink on a form, type its friendly text and its address, and have it open the page, so that I do not write a click handler.
- As a user, I want Ctrl/Cmd-click to open the page in a new tab and Shift-click in a new window, as on any website.
- As a developer, I want a `mailto:` link with a subject, a `tel:` link and a *download* link, because those are what `<a>` can do.
- As a security reviewer, I want `javascript:` addresses refused and external pages opened without access to my application.

## 4. Requirements (EARS)

**The control**
- **R1 (ubiquitous):** A WebLink control shall exist in the toolbox of web and mobile projects, with its own icon, default size and default caption, as a control type of its own that inherits the Label's
  properties, events and methods (Q1 on name and on desktop forms).
- **R2 (ubiquitous):** A WebLink shall show its **Caption** as link text (the "friendly text") with a link appearance — colour, and underline always, on hover or never (a property) — and shall use the
  **hand** cursor over it unless the developer chose another.

**Properties** (names are proposals for `/plan`)
- **R3 (ubiquitous):** The control shall have, besides the Label's: **Url**; **Target** (`Self`, `NewTab`, `NewWindow`, `Parent`, `Top`; HTML `_self`, `_blank`, …) with the window features for `NewWindow`
  (width, height, position, as `window.open` takes them); **Rel** (`noopener`, `noreferrer`, `nofollow`, `external`, … as a list); **Download** (off, or the file name offered to the user); **HrefLang**;
  **MimeType** (the `type` attribute); **ReferrerPolicy**; and the colours of the states **Link**, **Hover**, **Active** and **Visited** (the last styled by CSS only; the page cannot read whether
  a link was visited). **`ping` is not offered in v1** (operator, 2026-10-20): it is the one `<a>` attribute that makes the browser send a request to an address the page never fetches, which the Content-Security-Policy of WASM-013 R2 would block unless
  `connect-src` were widened, and which is hostile to the user's privacy; a later spec may add it, limited to the backend's origin.
- **R4 (ubiquitous):** The accessible name shall be the Caption; an optional accessible label property shall override it, and the existing Tooltip shall be the `title`.

**Behaviour**
- **R5 (event):** When the user clicks a WebLink, the control shall raise its click event first and then navigate as `Target` says. A plain click follows `Target`; **Ctrl/Cmd-click and middle-click** open a new
  tab; **Shift-click** a new window; **Alt-click** follows the browser's convention for downloading. The event shall tell the handler which modifier keys were down.
- **R6 (event):** When a handler sets the control's **Cancel** indication during the click event (a property read after the handler returns, Q3), navigation shall not happen.
- **R7 (ubiquitous):** The control shall accept `http`, `https`, `mailto` (with `subject`, `body`, `cc`, `bcc` parameters), `tel`, `sms`, relative URLs and fragment (`#name`) links, and shall **refuse**
  `javascript:`, `vbscript:` and `data:` addresses (except a `Download` of a generated file), whether set in the designer or at run time. The accepted schemes shall be configurable per project, never widened
  by default.
- **R8 (ubiquitous):** When `Target` is `NewTab` or `NewWindow` and `Rel` is empty, the control shall open the page with `noopener noreferrer`; the developer can loosen it explicitly.
- **R9 (ubiquitous):** The control shall be focusable and activate with Enter, show a visible focus ring, and offer the browser's context menu (open in new tab, copy link address) on the web.
- **R10 (event):** When a fragment link names a control or an anchor on the form, the form shall scroll to it and move focus there.
- **R11 (state):** While a WebLink runs inside the mobile shell, an address outside the application's own origin shall open in the **system browser** (not in the application's web view), and `mailto:`/`tel:`/`sms:`
  shall be handed to the operating system.

**Methods and events**
- **R12 (ubiquitous):** The control shall offer `SetUrl`, `GetUrl`, `Open` (acts as a plain click, honouring `Target`), `OpenIn(target)`, plus the Label's `SetCaption` and `GetCaption`; every name shall be in the closed
  method vocabulary (`methods.rs`) and in the System KB.
- **R13 (ubiquitous):** The control shall raise the Label's events and, in addition, a **before-navigate** event (where Cancel applies) and an **after-navigate** event for a same-context navigation.

**Designer and tooling**
- **R14 (ubiquitous):** In the Form Designer the control shall be drawn as link text; editing the Caption in place and the Url in the Properties panel (with scheme validation and an indication of a refused scheme)
  shall be possible; the Preview (WASM-008) is where the click is tried.
- **R15 (ubiquitous):** The control shall round-trip through `.cfrm`, have a `tests/controls/` test, be exercised by the tests that iterate `ControlType::ALL`, have its documentation in the compiler's KB tables and the
  regenerated `chunked.data`, a toolbox icon, and every user-facing string in six languages.

## 5. Acceptance criteria

- [ ] **AC1** — In a headless browser, a WebLink with Url `https://example.com` and Caption "Example" renders an `<a>` whose text is "Example" and whose computed `cursor` is `pointer`.
- [ ] **AC2** — A plain click navigates according to each `Target`; Ctrl/Cmd-click and middle-click open a new tab; Shift-click opens a new window; the click event reports the modifiers.
- [ ] **AC3** — With `Target` `NewTab` and empty `Rel` the opened page has no `window.opener`; with `Rel` set explicitly it is as set.
- [ ] **AC4** — `javascript:alert(1)`, `vbscript:` and `data:text/html,…` set in the designer and via `SetUrl` are refused, the control is left unchanged and the refusal is reported.
- [ ] **AC5** — `mailto:` with subject and body, `tel:`, a relative URL, a fragment link and a `Download` link each do what `<a>` does in the same browser.
- [ ] **AC6** — A handler that sets Cancel prevents the navigation; one that does not, does not.
- [ ] **AC7** — The control round-trips through `.cfrm` with every property at a distinct value; `ControlType::ALL`-driven tests pass; the KB freshness test passes.
- [ ] **AC8** — Keyboard: Tab reaches the link, Enter follows it, the focus ring is visible; an automated accessibility check reports no critical issue.
- [ ] **AC9** — In the mobile shell an external address opens the system browser and a `tel:` link opens the dialler (on a device or simulator).

## 6. Constraints & steering check

- **i18n:** yes — the toolbox label, tooltip, the Properties-panel labels and the refusal message, six languages.
- **Generated-code contract:** the control's events produce handlers in the generated COBOL like any control's; nothing new.
- **Docs:** `docs/` untouched on 2.x.x; the control's reference lives in the System KB until the guide chapter is written at release.
- **System KB:** **required** — all the tables named in §1 and the regenerated `chunked.data`; the freshness test is a gate.
- **Interpreter/binary parity:** any method (`Open`, `OpenIn`, `SetUrl`) that the interpreter implements for the hosts that run it must also be implemented by the code generator for the browser (WASM-010 R7, R17), which has no interpreter.
- **Fix vs feature:** **feature** (`features-2x`).
- **Safety:** R7 and R8 are the control's security model; the plan shall review them with the security specification proposed in WASM-INDEX.

## 7. Open questions

- **Q1 — Name and reach.** "WebLink" as the operator wrote it, or "Hyperlink"? And is the control available on **desktop** forms too (it would open the system browser, which needs a runtime capability that does
  not exist today)? Recommended: name as the operator wrote it; web and mobile only in 2.0.
- **Q2 — Alt-click and modifiers** differ between browsers and operating systems (Safari, macOS Command). Pin the table of conventions at `/plan` from the browsers the project supports.
- **Q3 — How a handler cancels.** A boolean property set during the event (`SET LINK-1::Cancel TO TRUE`), or a return value? Recommended: the property.
- **Q4 — Same-origin navigation inside a single-page application:** does a link to another form of the same app go through the router (WASM-INDEX) or a full page load? Out of scope here; the router spec decides.
- **Q5 — Visited styling** is privacy-restricted by browsers (only some properties can differ for `:visited`); state the supported subset.
- **Q6 — Percent-encoding and IDN:** does the control encode a URL the developer typed, and how does it show an internationalised domain? Recommended: pass the URL through the platform's URL parser and show the parsed form.
