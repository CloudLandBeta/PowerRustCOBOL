<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The mobile shell (Tauri 2): Android and iOS builds, devices, signing and permissions

- **Status:** draft (needs `/clarify`; statements about Tauri are from general knowledge and must be verified at `/plan`)
- **Folder:** specs/WASM-012-mobile-shell-tauri-2/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the first batch's list of missing specs (WASM-INDEX §5, "mobile shell"): toolchains, emulators, signing, permissions and device features — accepted by the operator on 2026-10-19. Item 1 of the original list
  names Tauri 2 as the mobile stack.
- **Depends on:** WASM-001 (the Mobile kind and its options), WASM-002 (the bundle), WASM-010 (the module). **Related:** WASM-011 (navigation on a phone), WASM-009 (secure storage for the refresh credential), WASM-013 (security).

## 1. Overview

A Mobile project's frontend is the same bundle a Web project builds (WASM-002): a host page, the generated module and the player that draws the forms. **Tauri 2** wraps it in a native application for **Android** and
**iOS**: a small native shell that opens the system web view on the bundled files, gives the page controlled access to device features, and produces the package the stores accept. This spec is about that wrapper — what the
developer needs installed, how they run and sign the application, how it asks for permissions and which device features a COBOL program can use.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **Nothing.** No `tauri` in any `Cargo.toml`; no Android or iOS logic in the compiler (`cobolt-compiler`), which builds for the host triple only and refuses any other target (`UnsupportedTarget`, spec 041 R17/R18).
  `toolchain.rs` checks the Rust compiler version and offers the rustup installer; it knows no Android SDK, NDK or Xcode.
- **The IDE already stores secrets in the operating system's store** (`cobolt-ide/src/secrets/`: keychain on macOS, credential store on Windows, the Secret Service on Linux), best-effort with a plaintext fallback — a design
  the mobile case must not inherit silently (R4).

### What Tauri 2 offers (general knowledge — verify each point at `/plan`)

Tauri 2 builds Android and iOS applications from a Rust project and a web frontend; the CLI has `android` and `ios` subcommands (`init`, `dev`, `build`); the application uses the platform's own web view (Android System
WebView, WKWebView); frontend assets are bundled into the application; native features come from **plugins** with a **capability** system that says which window may call what; Android needs Android Studio's SDK, the NDK, a
JDK and the Rust Android targets, iOS needs macOS, Xcode and the Rust iOS targets. Updates on mobile go through the stores.

## 2. Goals / Non-goals

- **Goals:**
  - From a Mobile project, an **Android package** and an **iOS package** built from the same frontend, from the IDE.
  - Honest toolchain detection with the exact next step for what is missing.
  - Run on an emulator, a simulator or a device, with logs in the IDE.
  - Signing with the developer's own credentials, kept out of the project files.
  - Permissions declared from what the application uses and requested at the right time.
  - A small, versioned set of device features COBOL can use.
- **Non-goals:**
  - Watch and TV form factors (WASM-INDEX, later).
  - Submitting to the stores (the IDE prepares and checks; submission stays manual).
  - Building iOS on Windows or Linux (Apple's tools require macOS).
  - Self-updating a mobile application outside the stores.
  - A native UI: the screens are the web player's.

## 3. User stories

- As a mobile developer, I want one button to build an Android package and, on a Mac, an iOS package, so that I do not learn two toolchains.
- As a developer, I want to run the application on the Android emulator or my own phone and see its log in the IDE.
- As a developer, I want the IDE to tell me which permission my program needs, instead of the application failing on a user's device.
- As a user, I want to be asked for the camera or my location when I use it, with a sentence that says why.

## 4. Requirements (EARS)

**Build and toolchain**
- **R1 (event):** When the developer builds a Mobile project, the IDE shall produce, for each platform the project selected, an Android package (installable file and the store's bundle format) and an iOS archive, wrapping the
  bundle of WASM-002 in the Tauri shell, with the application's identity taken from the project (R6).
- **R2 (event):** When a required tool is missing — the Android SDK, NDK or JDK, the Rust Android or iOS targets, Xcode and its command-line tools, the Tauri CLI at the version the project pins — the IDE shall say which, what it
  is for and how to obtain it, install only what is safe (`rustup target add`), and never install Xcode or the Android SDK itself. On a machine that cannot build a platform (iOS off macOS) the platform shall be offered as unavailable with
  the reason.
- **R3 (event):** When the developer runs the application on a device, the IDE shall list the emulators, simulators and connected devices, start the chosen one, install and launch the application, reload the frontend on a
  change, show the device log in the Output panel and stop it on request.
- **R3a (event, operator ruling 2026-10-23):** The IDE's **Preview and Run** of a Mobile project shall be R3's run on an emulator, simulator or device — the IDE has no
  other preview surface for mobile (WASM-008 §0, R14). When the machine has no emulator or simulator for the platform, they fall back to the browser at a phone
  width with a notice that it is not the device (WASM-008 R15).

**Identity and signing**
- **R4 (ubiquitous):** Release signing shall use the developer's own credentials — an Android keystore, an Apple developer certificate and provisioning profile — referenced by name from the project; **passwords and keys shall be
  held in the operating system's secret store and never written to a project file**, and where that store is unavailable the IDE shall refuse to continue rather than fall back to plaintext. Debug builds shall sign automatically.
- **R5 (ubiquitous):** The Build shall refuse to produce a release package that is unsigned or signed with a debug key, and shall say so.
- **R6 (ubiquitous):** The application's identity — a reverse-DNS application id, name (translatable in the six languages), version and build number derived from the project version, icons and splash screen — shall come from the
  project; the icons shall be generated at every size the platforms require from one source image in `Assets/`.

**Permissions and device features**
- **R7 (ubiquitous):** Each device feature the application uses shall be **declared** by the program (a capability the compiler finds in its calls and controls), and the Build shall write the platform declarations from them
  (Android permissions, iOS usage descriptions) and fail, naming the feature and the line, when a program uses a feature the project has not declared.
- **R8 (event):** When the application first needs a permission, it shall ask at that moment, with the developer's rationale text (translatable), and a refusal shall reach the program as a normal failure it can handle.
- **R9 (ubiquitous):** The device features COBOL can use shall be a **short, versioned set**, each a host-API entry (WASM-010 R12) with a permission, a documented meaning and a KB entry. The first set (Q1) is **secure storage** (WASM-009
  R5), **opening an address in the system browser or the dialler** (WASM-006 R11) and **deep links**; the others (camera, location, notifications, biometric unlock, haptics, share sheet, clipboard, barcode scanner) follow one at a time.

**Behaviour on the device**
- **R10 (ubiquitous):** The shell shall pass the platform's **safe areas** and the on-screen keyboard's size to the page as CSS environment values (WASM-007 R10), honour the orientation the project fixes, and deliver the system Back
  gesture or button to the navigation of WASM-011 R13.
- **R11 (event):** When the application goes to the background, returns or is terminated, the lifecycle events of spec 037 shall run as far as the platform allows; a returning application shall re-establish its backend connection
  (WASM-003 R8) and, if its session has expired, ask the user to sign in again.
- **R12 (ubiquitous):** The page shall be loaded from the application's bundle, never from the network; only the bundled frontend shall be allowed to call a native command (the capability list names exactly the commands it may call);
  addresses outside the application's origin shall open in the system browser; the web view shall run under the Content-Security-Policy of WASM-013.
- **R13 (ubiquitous):** The Build shall print what the stores will ask for — iOS privacy declarations, Android data-safety items — derived from the declared features, as a checklist.
- **R14 (constraint):** A Desktop or Web project shall be unchanged, and the Mobile kind's options in the New Project dialog (default orientation, minimum OS) shall be honoured.

## 5. Acceptance criteria

- [ ] **AC1** — On a machine with the tools, a Mobile project builds an Android package; on a Mac it also builds an iOS archive; each starts on an emulator or simulator and shows the project's main form.
- [ ] **AC2** — With each tool of R2 hidden in turn, the IDE names it and the next step; Xcode is never installed by the IDE; iOS is offered as unavailable on a non-Mac.
- [ ] **AC3** — Run on a device: the list shows emulators and devices, the application installs, a frontend change reloads, the device log appears in the Output panel.
- [ ] **AC4** — A release build without signing credentials is refused; with them it succeeds, and a scan of the project folder finds no keystore password or key.
- [ ] **AC5** — A program that calls a device feature without the project declaring it fails the Build at its line; declared, the platform files contain the matching permission and usage text.
- [ ] **AC6** — The permission prompt appears on first use with the rationale; a refusal arrives as a failure the program handles.
- [ ] **AC7** — Safe-area and keyboard insets reach the CSS; rotation honours the fixed orientation; the Back gesture pops the navigation stack.
- [ ] **AC8** — Backgrounding and returning raises the lifecycle events and re-establishes the connection; an expired session leads to sign-in.
- [ ] **AC9** — A page in the web view cannot call a native command that the capability list omits, and an external address opens in the system browser.
- [ ] **AC10** — Web and Desktop projects build exactly as before.

## 6. Constraints & steering check

- **i18n:** yes — the toolchain messages, device list, signing and permission texts of the IDE, six languages; the application's name and rationale texts are translatable by the developer.
- **Credentials:** R4 and AC4 apply the project's no-credentials-in-files rule; the signing material is the operator's, never the agent's.
- **Generated-code contract:** the shell project (Tauri configuration, platform manifests) is a build artefact generated from the project, never hand-edited; the planners decide whether it lives in the build folder (specs 100/101).
- **Tech steering:** Tauri and its plugins go through the dependency guards (the `aws-lc-rs`/`ring` ban, MSRV 1.92) and a licence check; **a Rust build per application is unavoidable on mobile** (the shell is compiled and signed per
  application), even though a Web build needs none (WASM-002 Q7, WASM-016 R1). The frontend inside the shell is the same player and program module.
- **Self-resizing windows:** the IDE's device list and signing dialogs follow the explicit-size rule.
- **Docs:** `docs/` untouched on 2.x.x.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — The first device features.** The minimum is what other specs need (secure storage, opening addresses, deep links). Which of camera, location, notifications, biometric unlock and barcode scanning belong to 2.0?
  **Answered (operator, 2026-10-22), to confirm:** "yes". *Whether all five features (camera, location, notifications, biometric unlock, barcode scanning) enter 2.0 is to confirm.*
- **Q2 — Cloud or remote builds.** iOS needs a Mac; a developer on Windows or Linux has no route. Out of scope, or is a remote-Mac build a later spec?
  **Decided (operator, 2026-10-22):** no remote builds: building for iOS needs a Mac.
- **Q3 — Minimum OS levels** the New Project dialog offers (Android API level, iOS version) and how they track Tauri's and the web views' own minimums.
  **Decided (operator, 2026-10-22):** the last version of 2024 of each operating system; each new major version of PowerRustCOBOL AI Web/Mobile raises the required OS by one year.
- **Q4 — Tablet layouts.** Android Tablet and iPad are separate templates in WASM-001; do they differ from phones only by the responsive breakpoints (recommended) or by a separate shell?
  **Decided (operator, 2026-10-22):** the recommendation.
- **Q5 — Pinning.** Tauri 2 and each plugin pinned per project with an upgrade path, or tracked with the IDE?
  **Decided (operator, 2026-10-22):** track the IDE.
- **Q6 — Certificate pinning and similar hardening** for the backend connection: optional, in WASM-013, or later.
  **Decided (operator, 2026-10-22):** optional, in WASM-013.
