<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Navigation and application styles: SPA by default, standalone forms as the alternative

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-011-navigation-and-application-styles/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the first batch's list of missing specs (WASM-INDEX §5, "navigation") and the operator's reply of 2026-10-19: *a single-page application is the default but not the only kind of application; there can be
  standalone forms as in the desktop version, opening standalone forms instead of SPAs.*
- **Depends on:** WASM-001 (project kinds and options), WASM-002 (the host page), WASM-010 (suspension). **Builds on:** spec 049 (the application shell, `FormFormat`, `super`), spec 037 (window lifecycle).

## 1. Overview

A desktop application opens forms as windows (`OpenFormSync` modal, `OpenFormAsync` modeless) or, with the application shell of spec 049, loads them into a ContentPane. A browser has neither a window manager nor a
blocking modal, but it has URLs, history and tabs. This spec says what a Web or Mobile project's forms do when one opens another. There are **two application styles**:

- **SPA (default):** one page, one WebAssembly application; forms are loaded into the shell's ContentPane or shown as in-page dialogs; the address bar follows the form being shown.
- **Standalone forms:** each form is a page of its own, as each is a window on the desktop; opening one opens it in a new tab or window (or, on a phone, pushes a screen), and it runs in its own instance.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The shell (spec 049):** when the main form carries a sidebar menu the application is one window — MenuPane, breadcrumb, ContentPane; forms load *into the pane*; a **`FormFormat`** property (`Standalone` | `Embedded` |
  `Both`) decides how a form may be loaded and is checked at build time against the load path; a chain of resident forms feeds the breadcrumb and the **`super`** receiver (the form that loaded or opened this one); a project
  without a sidebar menu keeps one window per form. Cross-form data sharing is a non-goal there: each form is its own run unit.
- **The verbs:** `OpenFormSync`/`OpenFormAsync` reach a window supervisor (`cobolt-runtime/src/interpreter.rs:1672`, `:2664`); `OPENSTANDALONEFORM` exists beside them (`:2464`); the form host embeds a form in the pane
  (`cobolt-form-host/src/host.rs:104-119`, a `Pane` mode against an own OS window).
- **Nothing browser-side exists:** no URL, history, popup or routing concept anywhere in the workspace.

## 2. Goals / Non-goals

- **Goals:**
  - A project-level **application style** (SPA | Standalone), with SPA the default.
  - Defined behaviour for every combination of style and verb (`OpenFormSync`, `OpenFormAsync`, a menu load), on the web and on mobile.
  - URLs and the browser's Back/Forward and reload for SPA; deep links to a form.
  - `super`, the lifecycle events and `FormFormat` meaning what they mean today, as far as a browser allows.
- **Non-goals:**
  - Cross-form data sharing (the shared run unit, `EXTERNAL`, `USING` on `OpenFormSync`): its own spec, as in spec 049.
  - A server-side router or server-rendered pages.
  - Animations between routes.
  - Changing the desktop window model.

## 3. User stories

- As a developer building a back-office application, I want one page with a menu and screens that load into it, with working Back and bookmarks, so that it behaves like a modern web application.
- As a developer porting a desktop application, I want `OpenFormAsync` to open the other form in its own window, as it did, so that I do not redesign the flow.
- As a user, I want to send a colleague the address of a screen, so that they land on it (after signing in).
- As a user on a phone, I want Back to return to the previous screen.

## 4. Requirements (EARS)

**The style**
- **R1 (ubiquitous):** The application style shall be a project setting chosen in the New Project dialog (WASM-001 R4) and changeable in the project's settings before a Build; **SPA** is the default.
- **R2 (ubiquitous):** The behaviour of the opening verbs shall be, in each style:

  | | `OpenFormSync` (caller waits) | `OpenFormAsync` (caller continues) | A menu load (shell) |
  |---|---|---|---|
  | **SPA** | an in-page **modal dialog**; the caller's handler is suspended (WASM-010 R9) until it closes | an in-page **modeless panel** | the form loads into the ContentPane (spec 049) |
  | **Standalone** | a **new tab or window** (a screen pushed, on mobile); the caller is suspended until it closes | a **new tab or window**; the caller continues | not used (no shell) |

- **R3 (ubiquitous):** `FormFormat` (spec 049) shall keep its meaning and its build-time check: a form that is `Embedded` cannot be the target of a standalone open and one that is `Standalone` cannot be loaded into a pane. In an SPA, a `Standalone` or `Both` form opened by `OpenFormSync`/`OpenFormAsync` is shown as the in-page dialog or panel of R2; that is **not** a
  load into the ContentPane, so it does not make the form `Embedded`.

**SPA**
- **R4 (event):** When the SPA shows a form, the address shall change to that form's **route** (the form's name plus the parameters it was opened with); Back and Forward shall move through the navigation chain; a reload or a
  pasted address shall open the application at that form (Q1 on what is restored). The form's **data region** in the project's program module is created when the form is created and released when the form is destroyed — not while it is resident or preserved by `PreservePreviousForm` (WASM-017 R2, R5); the module itself is loaded once, at start.
- **R5 (ubiquitous):** The route form shall work on any static host without server configuration (Q2) and shall not hijack the host page's address when the application is embedded in a layout (WASM-002 R11).
- **R6 (event):** When an address names no form, or a form the user may not open, the application shall show a defined **not-found** or **sign-in** outcome (WASM-009 R11) and raise an event the developer can handle.
- **R7 (ubiquitous):** On every navigation the focus shall move to the new form's title, the change shall be announced to assistive technology, and the scroll position shall reset.
- **R8 (optional):** Where a form marks itself as holding unsaved changes, leaving it by Back, a menu or closing the tab shall offer the user a confirmation, within what the browser allows.

**Standalone**
- **R9 (ubiquitous):** Each standalone form shall have its **own address and page**, openable directly (a bookmark, a link) subject to sign-in, and shall run in **its own instance** — its own data, as on the desktop.
- **R10 (event):** When a standalone open comes from a user's gesture, the new tab or window shall open; when the browser's popup blocker refuses it (a call from a timer or after a wait), the application shall tell the
  user how to allow it and raise an event, and the suspended caller shall be released with a failure, not left hanging.
- **R11 (ubiquitous):** `super` shall work in a standalone form as the form that opened it: reading and writing its properties and calling its methods shall go through messages between the two windows of the same
  origin, shall be asynchronous from the platform's point of view and synchronous from the program's (the caller is suspended meanwhile), and shall fail with `ON EXCEPTION` when the opener has been closed (Q3).
- **R12 (event):** When a standalone window is closed, its lifecycle events (spec 037) shall run as far as the browser allows, and a caller suspended on `OpenFormSync` shall be released.

**Mobile**
- **R13 (state):** While the application runs in the mobile shell (WASM-012), there are no tabs or windows: standalone forms shall be **screens pushed on a navigation stack**, the system Back gesture or button shall pop it, and
  a deep link (the operating system's address scheme) shall open a form as in R4/R9.

**General**
- **R14 (ubiquitous):** Modal dialogs shall trap focus, be closable by the keyboard where the form allows it, stack in the order opened, and block interaction with what is behind them.
- **R15 (ubiquitous):** A Desktop project shall be unchanged.

## 5. Acceptance criteria

- [ ] **AC1** — A SPA with a SideMenu and three forms: choosing a menu entry loads the form and changes the address; Back returns to the previous form; a reload at a form's address reopens it.
- [ ] **AC2** — `OpenFormSync` in an SPA shows a modal dialog and the caller does not run on until it closes (a test checks the statement after the verb); `OpenFormAsync` shows a modeless panel and the caller runs on.
- [ ] **AC3** — A standalone project: `OpenFormAsync` from a click opens a new tab (a headless browser sees it); `OpenFormSync` suspends the caller until that tab closes, then resumes it.
- [ ] **AC4** — A standalone open from a timer, with popups blocked, releases the caller with a failure and shows the hint.
- [ ] **AC5** — `super` property reads and method calls cross from a standalone child to its opener; closing the opener makes the next one fail with `ON EXCEPTION`.
- [ ] **AC6** — An unknown address shows the not-found outcome; a protected form's address without a session shows sign-in and, after sign-in, the requested form.
- [ ] **AC7** — Focus lands on the new form's title on each navigation and a screen reader announces it (an automated accessibility check).
- [ ] **AC8** — A form marked as having unsaved changes prompts on Back and on closing the tab.
- [ ] **AC9** — In the mobile shell, standalone forms stack, Back pops them and a deep link opens a form.
- [ ] **AC10** — The `FormFormat` build-time check rejects an `Embedded` form opened standalone, and the desktop projects' tests stay green.

## 6. Constraints & steering check

- **i18n:** yes — the popup hint, the not-found text and the unsaved-changes wording, six languages; the application's own texts are the developer's.
- **Generated-code contract:** routes and entry pages are generated from the form list, never hand-edited.
- **System KB:** new behaviour of `OpenFormSync`/`OpenFormAsync`, `super` and any new event (not-found, popup-blocked, leave-confirmation) goes into the compiler's documentation tables and `chunked.data`.
- **Self-resizing windows:** a standalone popup's size comes from the form's designed size and changes only if the user drags it; the application never resizes its own window.
- **Docs:** `docs/` untouched on 2.x.x.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — What a reload or a deep link restores.** The form and its opening parameters, not its in-memory data (recommended); or also a saved snapshot of the data in `sessionStorage` (never tokens, WASM-009 R5)?
- **Q2 — Route syntax.** A hash (`#/customer/42`) works on any static host and does not touch the host page's path; a path (`/customer/42`) is cleaner but needs the server to serve the application for unknown paths.
  Recommended: hash by default, path as an option.
- **Q3 — `super` between windows** needs a channel the browser offers only within one origin and one browser profile; confirm that limit is acceptable, and that a closed opener is a normal `ON EXCEPTION`.
- **Q4 — Same-window multi-page navigation** (each `OpenForm` replacing the page, a classic multi-page site) is a third style the operator did not name: out of scope, or part of "standalone"?
- **Q5 — Mobile default style:** SPA with a stack (recommended) or standalone screens throughout?
- **Q6 — The program module of a standalone page — decided** (operator, 2026-10-20, WASM-017 Q1): every page loads **the project's one program module** and the one player, instantiated with the page's start form; the browser caches both once for all pages.
- **Q7 — Several instances of the application on one page** (embedded in two places): allowed or refused?
