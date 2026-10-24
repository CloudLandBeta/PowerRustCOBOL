<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The Preview is a web view

- **Status:** draft — **re-scoped by the operator on 2026-10-23** (§0): the interactive Preview and Run open in a browser, not in a web view of the IDE's own.
- **Folder:** specs/WASM-008-webview-preview/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **9** — the Preview shall be a web view.
- **Depends on:** WASM-001, WASM-002, WASM-004, WASM-007. **Related:** WASM-003 (the dev backend), WASM-009 (the dev sign-in).
- **Prior art:** spec 006's Preview was `trunk serve` plus the default browser (removed 2026-10-14). Spec 039 once considered `wry` for the Maps control and rejected it (`specs/039-*/plan.md:298`) because
  hand-rolled tiles made it unnecessary; the reason does not apply here.

## 0. Operator ruling (2026-10-23) — read first

> "o preview/run de apps mobile deve ser em emuladores android/iOS. Da mesma forma, preview de apps web deve ser no browser do sistema (ou um
> browser configurado nas configurações do projeto)." — the operator, 2026-10-23.

It overrides every requirement and acceptance criterion below that says otherwise, and §4 states it as R12–R16:

- **Web project:** Preview and Run open the form in the **system's default browser**, or in the browser the project names in its settings. No
  window of the IDE shows the page.
- **Mobile project:** Preview and Run go to an **Android emulator** or an **iOS simulator** (or a connected device), through the mobile shell of
  WASM-012. When the machine has no emulator or simulator for the platform, they **fall back to the browser** at a phone width, with a notice that
  says it is not the device (operator's answer, 2026-10-23).
- **The web-view helper (`prc-web-preview`, wry + tao, shipped at 2.0.20) stays, headless only:** it is the engine behind `render_form`, `run_form`
  and the tests (R9; WASM-015 Q1). Its visible window and its controls window go (operator's answer, 2026-10-23).
- **Device presets, orientation, pixel ratio and safe-area insets are not the IDE's on the web:** the browser's own responsive/device tools cover
  them, and a mobile project is tried on the emulator (operator's answer, 2026-10-23).

## 1. Overview

The only honest preview of a web form is the web. So for Web and Mobile projects the Preview shows the form in a **web view** — the system's browser engine in a window of its own — running the same bundle the
Build produces, with the project's real CSS, fonts and DOM. The Form Designer's egui canvas stays the *drawing* surface; the web view is where the developer *tries* the form, on a phone-sized or a desktop-sized
viewport, and where a style change shows up as the browser draws it.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The desktop Preview** is an in-process secondary eframe viewport (`show_viewport_immediate`, `cobolt-ide/src/app.rs:16806`) drawn by `show_preview_window` (`app.rs:17327`) with `cobolt_forms::paint`.
  **Run Form** is an external process: `ExternalFormRun::spawn` (`form_runtime.rs:419`) starts `rcrun run-form` (`cobolt-cli/src/main.rs:78`, `form_gui.rs`), which hosts `cobolt-form-host` on eframe/egui 0.36.
- **Headless render exists for egui:** `--headless` (`form_gui.rs:772`, `cobolt-form-host/src/headless.rs`) renders a PNG on the CPU rasteriser; the MCP tools `render_form` (a picture) and `run_form` (a scripted run
  off screen) rely on it.
- **There is no web view or browser engine** anywhere: `wry`, `tauri`, `webview`, `leptos`, `wasm-bindgen` and `trunk` have no hits in any `Cargo.toml` or source file.

## 2. Goals / Non-goals

- **Goals:**
  - Preview of a web or mobile form in a web view, from the same pipeline as Build.
  - Live update on save and on a style change.
  - Device and viewport emulation, without the window ever resizing itself.
  - The developer tools of the web view available, and the page's console in the IDE's Output panel.
  - A headless counterpart for tests and agents.
- **Non-goals:**
  - ~~Android emulators, iOS simulators and on-device runs~~ — **now in scope** (§0, R14): the Preview of a Mobile project goes to them; the emulator and device machinery itself is WASM-012's (R3).
  - Replacing the Form Designer's canvas with a web view (a later, separate decision).
  - Remote device debugging.

## 3. User stories

- As a developer, I want Preview to show my form as a browser will, so that I trust what I see.
- As a developer, I want to flip between a phone, a tablet and a desktop width, and between portrait and landscape, so that I check my responsive layout.
- As a developer, I want a style change to show at once, so that I can adjust CSS quickly.
- As an agent, I want a screenshot and a scripted run of a web form from the tools, as I have for desktop forms.

## 4. Requirements (EARS)

- **R1 (event):** When the developer starts Preview on a web or mobile form, the IDE shall build the form's frontend with the pipeline of WASM-002 — emit the program module and the form descriptions and use the **prebuilt player** (WASM-016), so no Rust compilation is involved —
  and show it in a **web view** (the system engine: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) — not an approximation drawn by the egui painter.
- **R2 (ubiquitous):** The web view shall live in a window of its own and, if it hangs or crashes, shall not hang or crash the IDE (Q1 on process separation).
- **R3 (event):** When the form, a property, `CssClass`, `Style` or the stylesheet changes, the Preview shall update in place, keeping the form's state where it can and reloading where it cannot, within a time the
  plan states and the build reports. Because the player is already loaded and a change is only a new form description, stylesheet or program module, the player's in-place update (WASM-016 R16) is the mechanism, and the time is expected to be short.
- **R4 (ubiquitous):** The Preview shall offer **device presets** (a phone, a tablet, a desktop width), **orientation**, **device pixel ratio**, **safe-area insets** and a colour-scheme (light/dark) and locale choice, and a
  free size set by a grip. The window shall **never resize itself**: its size changes only when the developer picks a preset or drags the grip.
- **R5 (ubiquitous):** The web view shall expose the engine's developer tools (inspector, console) in debug use, and the page's console output and uncaught errors shall appear in the IDE's Output panel with the form
  and, where the engine gives it, the COBOL line.
- **R6 (event):** When the form calls a backend (WASM-005), the Preview shall use the project's backend started for development (WASM-003 R20) and shall start it on request; when none is running it shall say so
  rather than fail silently (Q3 on a stub).
- **R7 (event):** When the build fails, the Preview shall keep showing the last good version and show the errors over it, and the errors shall also be in the Output panel.
- **R8 (constraint):** The web view shall load only the project's own development origin; any other navigation shall be blocked or handed to the operating system's browser, and the page shall have no access to the
  IDE or to any native interface beyond what the form needs.
- **R9 (ubiquitous):** A headless render of a web form to an image, and a scripted run (type, click, wait, read a property, return the displayed text, errors and a picture), shall exist for tests and for the
  MCP tools `render_form` and `run_form`, with the same shape of answer they give for desktop forms.
- **R10 (state):** While the project kind is desktop, the Preview and Run Form shall be exactly as today.

**The operator's ruling of 2026-10-23 (§0) — these override R1, R2, R4, R5 and R8 where they differ**
- **R12 (event):** When the developer starts Preview or Run on a **web** form, the IDE shall build it as R1 says, serve it from the project's loopback development
  origin, and open that address in the **browser the project names** (a project setting, Q7) or, when none is named, in the **system's default browser**. The IDE
  shall open no window of its own for the page. A named browser that cannot be started shall be reported in the Output panel, and the system browser used instead.
- **R13 (state):** While the page is open in the browser, R3's in-place update shall reach it from the development server (the player's in-place update, WASM-016
  R16), and R5's console output and uncaught errors shall be relayed by the served page to the IDE's Output panel. The browser's own developer tools replace the
  helper's; R4's presets, orientation, pixel ratio and insets are left to the browser's responsive/device mode, and the IDE offers none for the web.
- **R14 (event):** When the developer starts Preview or Run on a **mobile** form, the IDE shall run it on an **Android emulator or iOS simulator** (or a connected
  device) through WASM-012 R3 — list, start the chosen one, install, launch, reload on a change, device log in the Output panel.
- **R15 (state):** While the machine has no emulator or simulator for the project's platform (WASM-012 R2's toolchain check finds none), Preview and Run of a mobile
  form shall **fall back to the browser** of R12 at a phone width, and the Output panel and the IDE's status shall say that this is the browser, not the device,
  and what to install to run on the emulator.
- **R16 (ubiquitous):** The web-view helper (`prc-web-preview`) shall remain only as the **headless** engine of R9 (`render_form`, `run_form`, tests; WASM-015 Q1);
  it shall open no visible window. R2 (crash isolation) applies to the browser and the emulator for free, and to the helper's headless runs as before.

**Layers (spec 091, adapted to HTML)**
- **R11 (state):** The Preview and the Run of a layered web form shall show what the program shows: every layer starts hidden (WASM-004 R15; spec 091 R35). The visibility the developer set on the designer's layer tabs is not part of the form description and is not carried into the Preview.

## 5. Acceptance criteria

- [ ] **AC1** — Preview of a hello-world web form opens a web view that shows it, and a click on its button changes what it shows.
- [ ] **AC2** — Changing a property, a class rule and the stylesheet each updates the open Preview in place; the measured times are reported in the Output panel.
- [ ] **AC3** — Choosing each preset and each orientation changes the viewport's size and `devicePixelRatio` as set; over 120 frames the window keeps its rectangle when no preset or grip is touched; after the grip is
  dragged it keeps the dragged size.
- [ ] **AC4** — A `console.error` in the page and an uncaught exception appear in the Output panel with the form name.
- [ ] **AC5** — A deliberately broken build leaves the last good Preview visible with the errors over it.
- [ ] **AC6** — Killing the web view's process leaves the IDE responsive and the Preview can be reopened.
- [ ] **AC7** — A link to another origin does not navigate the web view; it opens in the system browser (or is blocked, per the plan).
- [ ] **AC8** — The headless render and the scripted run work for a web form and return the same fields as for a desktop one (`render_form`, `run_form`); a reference form's image is stable across runs within a tolerance.
- [ ] **AC9** — A desktop project's Preview and Run Form behave as before: the existing tests stay green and no new window or action appears for the desktop kind.
- [ ] **AC10** — Opening a layered form in the Preview shows none of its layers until the program shows one (R11).

**Acceptance under the ruling of 2026-10-23** (AC1, AC3, AC6 and AC7 above are read through these)
- [ ] **AC11** — Preview of a hello-world **web** form opens it in the system's default browser; with a browser named in the project settings, in that one; a click on
  its button changes what it shows; no IDE window shows the page.
- [ ] **AC12** — With the page open in the browser, changing a property, a class rule and the stylesheet each updates it in place (AC2's times), and a
  `console.error` and an uncaught exception appear in the Output panel with the form name (AC4).
- [ ] **AC13** — Preview of a **mobile** form on a machine with an emulator (or simulator) starts it, installs and shows the form, and the device log reaches the Output panel.
- [ ] **AC14** — On a machine with neither, Preview of a mobile form opens the browser at a phone width and the Output panel says it is not the device and what to install.
- [ ] **AC15** — `render_form` and `run_form` on a web form still answer from the headless helper (AC8), and no visible helper window opens in any Preview or Run.
- AC3 (presets, the window's own rectangle) and AC6 (killing the web view's process) **no longer apply** to the interactive Preview; AC7 is the browser's own behaviour.

## 6. Constraints & steering check

- **i18n:** yes — presets, orientation, colour-scheme and error text, six languages.
- **Self-resizing windows:** R4 and AC3 apply the golden rule (explicit size, one grip, children laid out from the stored size).
- **Platform dependencies:** WebKitGTK on Linux and WebView2 on Windows are runtime dependencies of the IDE for this feature; the installers (`installer-look`, the `.deb`/`.rpm` text) and the toolchain
  check must say so. The plan shall also check the licences of whatever web-view crate it picks and whether it lines up with the one Tauri 2 already uses (WASM-002).
- **Generated-code contract:** no change; the Preview consumes the Build's output.
- **Docs:** `docs/` untouched on 2.x.x.
- **System KB / agent kit:** the MCP tools' descriptions and the plugin's rules (`cobolt-project-tools/src/content.rs`) shall mention the web Preview.
- **Never drive the IDE window:** verification of this feature is by the MCP tools and headless runs, not by automating the IDE (operator ruling, 2026-07-15); whether it *looks* right stays with the operator.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — In-process or separate process.** An IDE window that embeds the web view as a child is possible on some platforms and awkward on others (the egui/eframe window is not a native container on every OS);
  a separate window or process (as Run Form already is) isolates crashes and is the same on all three systems. Recommended: a separate process and window.
- **Q2 — Which engine library,** and whether the IDE's Preview and the mobile shell use the same one. Decide at `/plan` with the crate probe (licence, MSRV 1.92, Linux dependencies).
- **Q3 — Backend during Preview.** The real backend started locally (recommended), a generated stub that answers from fixtures (fast, offline, but a second implementation to keep true), or both?
- **Q4 — Sign-in during Preview** (WASM-009): the developer signs in to a real provider, or a development token is allowed (WASM-003 Q5)?
  **Decided (operator, 2026-10-22):** a development token may be used.
- **Q5 — Update budget.** A number for R3 ("a style change shows within N ms") is needed before the AC can be checked; measure a prototype first.
  **Answered (operator, 2026-10-22), to confirm:** "as short as possible". A number is still needed before the acceptance criterion can be checked.
- **Q7 — The project's browser setting** (R12): its name and carrier (proposal: `[web] preview_browser`, a browser name or an application path, empty = the
  system default) and whether a Mobile project's fallback (R15) uses it too (proposal: yes). Decide at `/plan`.
- **Q6 — Does Preview replace Run Form** for web kinds, or are they two actions (Preview = quick, Run = the full bundle with the backend)? Recommended: two, as on desktop.
