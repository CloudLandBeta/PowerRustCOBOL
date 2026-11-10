<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# WASM series — acceptance criteria still open

> Written 2026-11-09. Every acceptance criterion (AC) of a `specs/WASM-*/spec.md` whose box is **not ticked**, with the task or test that was to verify it.
> **A closed task is not a verified criterion.** On 2026-11-09 the operator ruled that every task depending on one of these ACs is closed, so the work could go on; the criteria stay open **here**, and this document is where they are tracked.

## How to use this document when something goes wrong

1. Find the area in the index below, then the AC that describes the behaviour that failed. If it is listed, nobody has proved it yet: treat the failure as expected until the AC is audited.
2. Read the **Verified in** column: the task (in that spec's `tasks.md`) or test that was meant to prove it, and the test's name where there is one.
3. Reproduce with that test or, when none exists, write one first (a test that prints the quantified result block, GOLDEN RULE #7).
4. Fix on a `fixes-2x` branch (fix, never a feature), forward-port rules as in CLAUDE.md.
5. Tick the box in the spec's `spec.md`, **remove the row here**, and say in the CHANGELOG entry which AC the change closed.

## The classes

| Class | Meaning |
|---|---|
| `unaudited` | The implementing tasks are done and their tests exist and pass; the box was never ticked against the code. An audit (read the AC, find the test that proves it, run it) closes it. No hardware needed. |
| `blocked-operator` | Needs the operator's eyes or accounts (a look at a window, a real identity provider, an emulator comparison). |
| `blocked-hardware` | Needs a browser, device or simulator this Mac does not have (Safari, iOS, Android WebView, a physical phone; Firefox was removed from the targets on 2026-11-10). |
| `blocked-tool` | Needs a tool that is not installed here (`xcodegen`, an iOS 18 simulator). |
| `red` | Audited, and a test of the criterion's own suite fails for a reason found in the audit. Fix the cause (or document the difference where the test has a table for it), then tick. |

## Index

| Spec | Open ACs | Class |
|---|---|---|
| [WASM-001-project-types-and-ide-entries](#wasm-001-project-types-and-ide-entries) | 2 | `blocked-tool`, `red` |
| [WASM-002-wasm-build-target-and-host-page](#wasm-002-wasm-build-target-and-host-page) | 3 | `blocked-hardware` |
| [WASM-003-backend-project-and-secure-channel](#wasm-003-backend-project-and-secure-channel) | 11 | `unaudited` |
| [WASM-004-control-reuse-on-the-web](#wasm-004-control-reuse-on-the-web) | 13 | `unaudited` |
| [WASM-005-websocket-call-and-remote-data](#wasm-005-websocket-call-and-remote-data) | 11 | `unaudited` |
| [WASM-006-weblink-control](#wasm-006-weblink-control) | 10 | `unaudited` |
| [WASM-008-webview-preview](#wasm-008-webview-preview) | 15 | `unaudited` |
| [WASM-009-authentication-control-and-identity-providers](#wasm-009-authentication-control-and-identity-providers) | 9 | `unaudited` |
| [WASM-010-webassembly-code-generation](#wasm-010-webassembly-code-generation) | 11 | `unaudited` |
| [WASM-011-navigation-and-application-styles](#wasm-011-navigation-and-application-styles) | 1 | `unaudited` |
| [WASM-012-mobile-shell-tauri-2](#wasm-012-mobile-shell-tauri-2) | 12 | `blocked-hardware` |
| [WASM-013-security-architecture](#wasm-013-security-architecture) | 12 | `unaudited` |
| [WASM-014-backend-contract-and-evolution](#wasm-014-backend-contract-and-evolution) | 11 | `unaudited` |
| [WASM-015-testing-agent-tooling-and-knowledge](#wasm-015-testing-agent-tooling-and-knowledge) | 13 | `unaudited` |
| [WASM-016-prebuilt-player](#wasm-016-prebuilt-player) | 14 | `unaudited` |
| [WASM-017-module-architecture-and-js-boundary](#wasm-017-module-architecture-and-js-boundary) | 15 | `unaudited` |
| [WASM-018-canvas-and-graphics-objects](#wasm-018-canvas-and-graphics-objects) | 14 | `unaudited` |
| [WASM-036-stylesheet-values-in-designer-and-properties](#wasm-036-stylesheet-values-in-designer-and-properties) | 1 | `blocked-operator` |
| **Total** | **178** | |

## WASM-001-project-types-and-ide-entries

**2 open.** AC1–AC6 were audited against the code on 2026-11-10 (2.0.402) and ticked in `spec.md`; their rows are gone. AC7 was audited the same day and is not green (below). AC8 is the Xcode half of the native-project step (iOS), which needs WASM-012's iOS generator, which needs `xcodegen` and an iOS simulator; this Mac has Xcode 15.2 without iOS 18.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC7 | `red` | The existing desktop test suites are unaffected (no new failure in `cargo test -p cobolt-ide --bin cobolt-ide` and `-p cobolt-project-tools`). | T13, T18. **Sweep of 2026-11-10 (2.0.402, no code changed):** `cobolt-ide --bin` 2012 passed, 5 failed, 6 ignored: exactly the five known reds (`docs_embed::every_document_ships_in_every_language`, `external_crates_service::flow_tests::{live_resolver_verdicts, compatible_and_reserved_collisions_still_refuse}`, `grace_host::tests::{prebuilt_chunked_kb_matches_the_published_documentation, a_menu_reorder_request_retrieves_the_sidemenu_structure}`). `-p cobolt-project-tools`: every target green except **one new red**, `tests/desktop_comparison_chrome.rs::the_reference_form_on_the_desktop_and_in_chrome` (WASM-004 T33 / WASM-015 AC5): the CH-BAR colour probes differ by up to 112 levels at (95,370) and (245,370), the same numbers on four runs (deterministic, not load). The two pictures of the bar chart are alike but the desktop's tallest bar is about 12 px shorter and its baseline sits about 2 px lower than Chrome's. Not caused by WASM-001 (no chart code is in its tasks); not bisected; `AnimateOnLoad=false` on the chart did not change a number. Ticks when that test is green. |
| AC8 | `blocked-tool` | Creating a Mobile project with the native-project step accepted yields the Android Studio project (and, on a Mac, the Xcode project) named after the project, and the manifest says so; cancelling the step yields the Mobile project alone and a manifest that says the native project is missing; a Web or Desktop template shows no… | not mapped in its tasks.md |

## WASM-002-wasm-build-target-and-host-page

**3 open**, all `blocked-hardware`: Safari, iOS and Android WebView cells of the matrix cannot be measured here (Firefox is out of scope by the operator's ruling of 2026-11-10: global share below 3 %, about 1.9 % in the U.S.); the Mobile build is WASM-012.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC4 | `blocked-hardware` | A program in the subset, run on the desktop runtime and as the generated module (headless), prints identical output for a fixed set of programs; the conformance list and its numbers are WASM-010 R16 and AC7–AC8, which is what R4's "behave as" is measured against. | WASM-010 parity gate (not here) |
| AC6 | `blocked-hardware` | Missing-tool messages for each tool of R6 appear when that tool is absent (tested by hiding it from `PATH`). | T18 (Web half); Mobile half WASM-012 |
| AC7 | `blocked-hardware` | The Mobile build produces an Android package and (on macOS) an iOS package from the same project; both start and show the form. **Android half measured 2026-11-10:** the APK builds (42 MB, 12 s) and shows the form on the emulator; **iOS half open** (no `xcodegen`, no iOS 18 SDK; and no single-memory module for Safari / the iOS web view, WASM-017 R9). | T23 → WASM-012 |

## WASM-003-backend-project-and-secure-channel

**11 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A backend with one exported and one non-exported program builds to a single executable; a WebSocket client can call the first and gets a *not found* error for the second, whatever the spelling it tries. | T15b-3, T15b-4, T15b-6 |
| AC2 | `unaudited` | A call without a token is refused; with a token signed by a different key, with `alg: none`, expired, for another audience or issuer, or an identity token instead of an access token, is refused; a good token is accepted (a local test issuer supplies the keys). | D-b, T7 (re-run unchanged) |
| AC3 | `unaudited` | A program declared for role `admin` refuses a caller without it before the program starts, and runs for one with it; the program can read the caller's subject through the built-in. | D-b, D-d, T5 (claims re-installed per call), T7 |
| AC4 | `unaudited` | A connection from a non-allowed Origin is refused; `ws://` to a non-loopback address is refused. | T4, T7 |
| AC5 | `unaudited` | Payloads with an unknown field, a numeric field holding text, a string longer than its `PIC`, or a table longer than `OCCURS` are refused with a validation error and the program does not run. | D-b, T7 |
| AC6 | `unaudited` | Oversized frames, a call that exceeds the timeout and a client over the rate limit are each refused or cut off with a distinct error. | D-b, T7 |
| AC7 | `unaudited` | Two clients calling the same program at once get their own results (a test with interleaved calls and distinct data). | D-b, T5 (`per-call` unchanged, `per-connection` per connection) |
| AC8 | `unaudited` | The log of a run containing a token and a password-marked field contains neither. | D-b, T5 (log gains only the mode), T7 |
| AC9 | `unaudited` | The IDE starts and stops the backend and shows its log; the standalone binary behaves identically. | D-e, T10 |
| AC10 | `unaudited` | A secret planted in a frontend form stops the frontend build (WASM-002 AC5) while the same value in the backend configuration does not. | T12 |
| AC11 | `unaudited` | A capture of the channel between a frontend and its backend, taken at the TLS endpoint (a terminating proxy), holds no readable frame: every call and answer is ciphertext under the key pair generated at this start of the application, and a restart of the application yields a different key pair, so the capture does not decrypt… | T15b-2 (vectors, per-connection keys), T15b-3 (keys at start), T15b-4 (`the_channel_is_ciphertext_at_a_terminating_proxy`) |

## WASM-004-control-reuse-on-the-web

**13 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A table in one place lists each of the 62 controls with its class; a test fails when `ControlType::ALL` and the table disagree. | not mapped in its tasks.md |
| AC2 | `unaudited` | For each class A control a headless browser shows the expected element and the property round-trip test passes. | not mapped in its tasks.md |
| AC3 | `unaudited` | For each class B control the same data (a fixed series, a fixed value) renders in the browser and on the desktop engine, and the documented differences are the only ones a comparison finds. | not mapped in its tasks.md |
| AC4 | `unaudited` | A web project's toolbox lists no class D control; a form containing one makes Check fail with its line. | not mapped in its tasks.md |
| AC5 | `unaudited` | A desktop form opened as a web form keeps every property value (including the approximate ones) after a save and reload. | not mapped in its tasks.md |
| AC6 | `unaudited` | Click, change, focus and key events fire in the same order on the desktop and in the browser for a scripted interaction on each class A control. | not mapped in its tasks.md |
| AC7 | `unaudited` | A keyboard-only walk through a reference form reaches and operates every control; an automated accessibility check reports no critical issue. | not mapped in its tasks.md |
| AC8 | `unaudited` | `prebuilt_chunked_kb_matches_the_published_documentation` is green with the new KB entries. | not mapped in its tasks.md |
| AC9 | `unaudited` | A layer control with `ZOrder` 0 paints above a base control with `ZOrder` 209, in a browser test (R14). | not mapped in its tasks.md |
| AC10 | `unaudited` | A layer that starts hidden paints nothing, takes no pointer event and no focus, and `SET … Visible TO TRUE` shows it on the next frame (R15, R22). | not mapped in its tasks.md |
| AC11 | `unaudited` | A click at an opaque point of a layer background is discarded and does not reach a control beneath it; the same click at a transparent point reaches that control (R16). | not mapped in its tasks.md |
| AC12 | `unaudited` | Tab walks the base controls in order, then each visible layer upward, each in its own order (R20). | not mapped in its tasks.md |
| AC13 | `unaudited` | A `DataGrid` in a layer at a rounded corner of the form paints inside the arc (R17). | not mapped in its tasks.md |

## WASM-005-websocket-call-and-remote-data

**11 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | Parser and AST: the new phrase parses in all combinations of R1; every existing `CALL` test still passes; the new phrase with an elementary argument is rejected (or accepted, per Q1). | T1 |
| AC2 | `unaudited` | Round trip: for a reference group with nested groups, `OCCURS` and `DEPENDING ON`, signed and decimal fields, `FILLER`, an 88-level, `COMP` and `COMP-3` items and a `REDEFINES`, group → JSON → group is the identity; the JSON text is pinned in the test (names, order, number text, trimmed strings). | T1 (`cobolt-wire` vectors, `test_group_json_vectors`) |
| AC3 | `unaudited` | Each failure of R6 (unknown key, wrong type, overflow, over-long string, over-long table) is reported with the item name and writes nothing. | T1 |
| AC4 | `unaudited` | Against a loopback backend: `BY VALUE` leaves the caller's group unchanged; `BY REFERENCE` changes it; `RETURNING` fills the named group; both together fill both. | T1 |
| AC5 | `unaudited` | With a stopped backend, a wrong token, an expired token (refreshed once, then repeated), a timeout, an unknown function and an error in the remote program, `ON EXCEPTION` runs and the failure class is readable; with none of them, `NOT ON EXCEPTION` runs. | T1 (built-in); T5 (second mechanism, blocked on O1); T3 (payload limit as a failure) |
| AC6 | `unaudited` | During a pending call the form repaints and a click on it is queued and handled afterwards, in order. | T4 |
| AC7 | `unaudited` | A web form with a DataGrid bound to an indexed file and another bound to an SQL query fills, edits and saves through the backend and shows the same rows the desktop shows for the same data. **Amended (operator, 2026-10-23, T9 choice (b)):** for the indexed-file grid, "the same rows" holds except three recorded differences — a… | T9, T10, T15 (T16 for the player) |
| AC8 | `unaudited` | A browser that sends SQL text, a file path or a URL to a generated endpoint gets a refusal; the backend log shows no execution (R17). | T6 (structural), T9, T12, T17 |
| AC9 | `unaudited` | A web form carrying a connection string or key for a class C control stops Build; the same configuration on the backend side builds. | T13 (with T8, T11) |
| AC10 | `unaudited` | `cargo test` for the parser, AST, semantic and runtime crates is green; the System KB gains the phrase and the new built-ins and the freshness test passes. | T1, T18, T19 |
| AC11 | `unaudited` | A call in flight when the backend goes away waits through the retries; with the backend back in time, a safe-to-repeat program's call completes and another program's call reports *transport* with "may have been received"; with the backend gone for good, `ON EXCEPTION` runs once the third attempt fails, with the class *transpo… | T18b (with WASM-003 T15a) |

## WASM-006-weblink-control

**10 open.** Not audited against the code, one criterion at a time. Blocked part: WebKit half and Alt-click download not measured; the operator's desktop checks; the Mobile shell is WASM-012.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | In a headless browser, a Hyperlink with Url `https://example.com` and Caption "Example" renders an `<a>` whose text is "Example" and whose computed `cursor` is `pointer`. | T3, T8, T18 |
| AC2 | `unaudited` | A plain click navigates according to each `Target`; Ctrl/Cmd-click and middle-click open a new tab; Shift-click opens a new window; the click event reports the modifiers. | T5, T10, T12, T18 |
| AC3 | `unaudited` | With `Target` `NewTab` and empty `Rel` the opened page has no `window.opener`; with `Rel` set explicitly it is as set. | T1/T2 (kept), T5, T10, T18 |
| AC4 | `unaudited` | `javascript:alert(1)`, `vbscript:` and `data:text/html,…` set in the designer and via `SetUrl` are refused, the control is left unchanged and the refusal is reported. | T5, T13, T15 (and T1, T4 in the player) |
| AC5 | `unaudited` | `mailto:` with subject and body, `tel:`, a relative URL, a fragment link and a `Download` link each do what `<a>` does in the same browser. | T5, T9, T18 |
| AC6 | `unaudited` | A handler that sets Cancel prevents the navigation; one that does not, does not. | T7, T10, T13, T18 |
| AC7 | `unaudited` | The control round-trips through `.cfrm` with every property at a distinct value; `ControlType::ALL`-driven tests pass; the KB freshness test passes. | T6, T16, T17, T21 |
| AC8 | `unaudited` | Keyboard: Tab reaches the link, Enter follows it, the focus ring is visible; an automated accessibility check reports no critical issue. | T8, T12, T18 |
| AC9 | `unaudited` | In the mobile shell an external address opens the system browser and a `tel:` link opens the dialler (on a device or simulator). | T20 (blocked on WASM-012) |
| AC10 | `unaudited` | On a desktop form, a click on a Hyperlink opens its address in the system browser in all three hosts, and a refused scheme stays refused (R16). | T11, T13, T14, T19 |

## WASM-008-webview-preview

**15 open.** Not audited against the code, one criterion at a time. Blocked part: the operator's one-time visual check of the Web-view Preview (T11).

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | Preview of a hello-world web form opens a web view that shows it, and a click on its button changes what it shows. | T35, T47; T11 the look |
| AC2 | `unaudited` | Changing a property, a class rule and the stylesheet each updates the open Preview in place; the measured times are reported in the Output panel. | T33, T36, T47 |
| AC3 | `unaudited` | Choosing each preset and each orientation changes the viewport's size and `devicePixelRatio` as set; over 120 frames the window keeps its rectangle when no preset or grip is touched; after the grip is dragged it keeps the dragged size. | **no longer applies** (spec §5); T6 (done) history only |
| AC4 | `unaudited` | A `console.error` in the page and an uncaught exception appear in the Output panel with the form name. | T32, T33, T36, T47 |
| AC5 | `unaudited` | A deliberately broken build leaves the last good Preview visible with the errors over it. | T34, T47 |
| AC6 | `unaudited` | Killing the web view's process leaves the IDE responsive and the Preview can be reopened. | **no longer applies** to the interactive Preview (spec §5); the helper half: T4 (done), T40 |
| AC7 | `unaudited` | A link to another origin does not navigate the web view; it opens in the system browser (or is blocked, per the plan). | the browser's own behaviour (spec §5); the headless helper keeps `may_navigate`: T4 (done), T40 |
| AC8 | `unaudited` | The headless render and the scripted run work for a web form and return the same fields as for a desktop one (`render_form`, `run_form`); a reference form's image is stable across runs within a tolerance. | T41, T43 |
| AC9 | `unaudited` | A desktop project's Preview and Run Form behave as before: the existing tests stay green and no new window or action appears for the desktop kind. | T7 (done), T35, T43, T48 |
| AC10 | `unaudited` | Opening a layered form in the Preview shows none of its layers until the program shows one (R11). | T44 |
| AC11 | `unaudited` | Preview of a hello-world **web** form opens it in the system's default browser; with a browser named in the project settings, in that one; a click on its button changes what it shows; no IDE window shows the page. | T30, T31, T35, T37, T47; T11 |
| AC12 | `unaudited` | With the page open in the browser, changing a property, a class rule and the stylesheet each updates it in place (AC2's times), and a `console.error` and an uncaught exception appear in the Output panel with the form name (AC4). | T32, T33, T36, T47 |
| AC13 | `unaudited` | Preview of a **mobile** form on a machine with an emulator (or simulator) starts it, installs and shows the form, and the device log reaches the Output panel. | T39 (hand-over by a fake) — **the real run waits for WASM-012**; T11 |
| AC14 | `unaudited` | On a machine with neither, Preview of a mobile form opens the browser at a phone width and the Output panel says it is not the device and what to install. | T39, T47; T11 |
| AC15 | `unaudited` | `render_form` and `run_form` on a web form still answer from the headless helper (AC8), and no visible helper window opens in any Preview or Run. | T35, T40, T43, T48 |

## WASM-009-authentication-control-and-identity-providers

**9 open.** Not audited against the code, one criterion at a time. Blocked part: the operator's `render_form` and toolbox look; the OS secure store is WASM-012; AC2 needs a real Keycloak.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | The control shows each screen; the developer's choice of screens is honoured; strings translate; the axe-style accessibility check reports no critical issue. | T4, T5, T7, T9 (screens, Show flags, six languages); T25 (accessibility tree, O8) |
| AC2 | `unaudited` | Against the local Keycloak: create account, confirm, sign in, call an exported program with the resulting token (accepted), sign out; forgot-password and reset; MFA challenge when the realm requires it. | T26 (gated on `PR_KEYCLOAK_URL`, O2; the call step on O7) |
| AC3 | `unaudited` | A wrong password and an unknown user produce the same message and similar timing; a reset request for an unknown address looks like one for a known address. | T12 (`identity_enumeration.rs`) |
| AC4 | `unaudited` | With reCAPTCHA enabled, a request with no token, a replayed token, a token for another action or hostname and a score below the threshold are each refused; a good one is accepted (the provider is mocked). | T12 |
| AC5 | `unaudited` | After sign-in, `localStorage`/`sessionStorage` contain no token (a test reads them); the refresh credential is not visible to `document.cookie`; a forced expiry triggers one silent refresh and the call proceeds. | T17 (cookie attributes), T21 (refresh path), T25 (in Chrome) |
| AC6 | `unaudited` | A scan of the frontend bundle, the forms and the copybooks finds no client secret, reCAPTCHA secret or signing key; the same values in the backend configuration are accepted. | T10 (literal refused), T19 (page), T20 (exact-value scan) |
| AC7 | `unaudited` | Token strings do not appear in the backend's logs or the browser console for a full flow (a test greps both). | T12 (no field), T17 (log grep), T25 (console grep) |
| AC8 | `unaudited` | Each adapter passes the same adapter-conformance suite against its mock; the capability table lists what each does not support. | T13 |
| AC9 | `unaudited` | The control round-trips through `.cfrm`; `ControlType::ALL`-driven tests pass; the System KB freshness test passes. | T2, T6, T28 |

## WASM-010-webassembly-code-generation

**11 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A form whose button handler does `MOVE`, `ADD` and `DISPLAY` compiles to a module that, run on a headless engine with a stub host, produces the expected property writes and output; its import section lists only the host API. | not mapped in its tasks.md |
| AC2 | `unaudited` | A golden table of data layouts (a group with `REDEFINES`, `OCCURS … DEPENDING ON`, `COMP-3`, signed display numerics, `SEPARATE` signs) shows the memory image after initialisation and after `MOVE`s equal, byte for byte, to the expected image. | not mapped in its tasks.md |
| AC3 | `unaudited` | A table of arithmetic cases (rounding modes, `SIZE ERROR`, 18- and 38-digit operands, division with remainder) gives the interpreter's results on the module. | not mapped in its tasks.md |
| AC4 | `unaudited` | A program using `GO TO … DEPENDING`, `ALTER` and `PERFORM … THRU` with `EXIT PERFORM` runs identically on both engines. | not mapped in its tasks.md |
| AC5 | `unaudited` | A handler that makes a call that waits stops at it, the host resumes it later with an answer, and the final data equals the expected; two events raised meanwhile are queued and leave the data consistent. | not mapped in its tasks.md |
| AC6 | `unaudited` | A program with a file verb, an `EXEC RUST` block or an `EXEC SQL` statement fails to compile with the line and the alternative; a test fails if any `Stmt` variant or built-in lacks a table entry. | not mapped in its tasks.md |
| AC7 | `unaudited` | The NIST numbers for NC, IF and IC are recorded (compiled, passed, excluded with reasons), with 0 failures among those run. | not mapped in its tasks.md |
| AC8 | `unaudited` | The `tests/cobol/` corpus inside the subset prints the same output on both engines; any difference is an open bug entry. | not mapped in its tasks.md |
| AC9 | `unaudited` | The Build reports module size and start-up time; a module run against a host without a required API entry refuses to start. | not mapped in its tasks.md |
| AC10 | `unaudited` | The desktop build, the interpreter and the backend are unchanged: their existing tests stay green, and the backend still opens `redb` and files. | not mapped in its tasks.md |
| AC11 | `unaudited` | A program that writes `LAYER-9::Visible` on a form with one layer fails the build and names that layer (R19). | not mapped in its tasks.md |

## WASM-011-navigation-and-application-styles

**1 open.** One box left; see the spec.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC9 | `unaudited` | In the mobile shell, standalone forms stack, Back pops them and a deep link opens a form. | not mapped in its tasks.md |

## WASM-012-mobile-shell-tauri-2

**12 open**, all `blocked-hardware`: Mobile shell: the iOS half, the device measurements and the pinning rows need a device or simulator and the operator.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `blocked-hardware` | On a machine with the tools, a Mobile project builds an Android package; on a Mac it also builds an iOS archive; each starts on an emulator or simulator and shows the project's main form. | not mapped in its tasks.md |
| AC2 | `blocked-hardware` | With each tool of R2 hidden in turn, the IDE names it and the next step; Xcode is never installed by the IDE; iOS is offered as unavailable on a non-Mac. | not mapped in its tasks.md |
| AC3 | `blocked-hardware` | Run on a device: the list shows emulators and devices, the application installs, a frontend change reloads, the device log appears in the Output panel. | not mapped in its tasks.md |
| AC4 | `blocked-hardware` | A release build without signing credentials is refused; with them it succeeds, and a scan of the project folder finds no keystore password or key. | not mapped in its tasks.md |
| AC5 | `blocked-hardware` | A program that calls a device feature without the project declaring it fails the Build at its line; declared, the platform files contain the matching permission and usage text. | not mapped in its tasks.md |
| AC6 | `blocked-hardware` | The permission prompt appears on first use with the rationale; a refusal arrives as a failure the program handles. | not mapped in its tasks.md |
| AC7 | `blocked-hardware` | Safe-area and keyboard insets reach the CSS; rotation honours the fixed orientation; the Back gesture pops the navigation stack. | not mapped in its tasks.md |
| AC8 | `blocked-hardware` | Backgrounding and returning raises the lifecycle events and re-establishes the connection; an expired session leads to sign-in. | not mapped in its tasks.md |
| AC9 | `blocked-hardware` | A page in the web view cannot call a native command that the capability list omits, and an external address opens in the system browser. | not mapped in its tasks.md |
| AC10 | `blocked-hardware` | Web and Desktop projects build exactly as before. | not mapped in its tasks.md |
| AC11 | `blocked-hardware` | On a device, cutting the network during a session shows the modal with the device drawn, blocks touches, and restoring the network within the three attempts removes it with the form unchanged; cutting it for good shows the failure caption after the third attempt. | not mapped in its tasks.md |
| AC12 | `blocked-hardware` | On a machine with the Android SDK, creating a Mobile project for Android creates an Android Studio project named after the PowerRustCOBOL AI project, which Android Studio opens and runs on an emulator showing the main form after a Build; on a Mac with Xcode, the same for iOS. Cancelling the step at creation leaves no native p… | not mapped in its tasks.md |

## WASM-013-security-architecture

**12 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | The threat model exists, names every boundary of R1, and each row points to a control and a test; the `/analyze` of each later spec updates it. | not mapped in its tasks.md |
| AC2 | `unaudited` | A built bundle served under its generated CSP loads and runs, and the browser reports a violation for an injected inline script, an `eval` and a connection to an address outside `connect-src` (a headless test). | not mapped in its tasks.md |
| AC3 | `unaudited` | The `<meta>` and the header set are generated, and the Build prints which directives are only deliverable as headers. | not mapped in its tasks.md |
| AC4 | `unaudited` | A caption, item, cell and tooltip containing `<img src=x onerror=…>` render as text; a source search finds no forbidden sink in the player (the Viewer included); the Viewer's HTML with a script element, an inline event handler and a `javascript:` link renders without them, built from nodes. | not mapped in its tasks.md |
| AC5 | `unaudited` | A cross-site page cannot make the backend run a call using only the refresh cookie; a connection from a foreign Origin is refused; no wildcard origin is accepted together with credentials. | not mapped in its tasks.md |
| AC6 | `unaudited` | The secret scan finds each planted secret kind in the bundle, a form, a copybook and a stylesheet, fails the Build with file and pattern, and prints no value. | not mapped in its tasks.md |
| AC7 | `unaudited` | On a machine with no operating-system secret store, a signing or backend secret is refused rather than written to disk. | not mapped in its tasks.md |
| AC8 | `unaudited` | A dependency outside the allow-list, a crate with a known vulnerability and a disallowed licence each fail the test run; a Build writes an SBOM and integrity hashes. | not mapped in its tasks.md |
| AC9 | `unaudited` | Each unsafe switch of R11 shows a warning in the IDE and fails a release Build. | not mapped in its tasks.md |
| AC10 | `unaudited` | A full sign-in and call flow leaves no token, password or marked field in the backend log, the browser console or a crash report (a test greps all three). | not mapped in its tasks.md |
| AC11 | `unaudited` | The fuzz targets of R15 exist and run within their budget without a crash. | not mapped in its tasks.md |
| AC12 | `unaudited` | The threat model names the channel's encryption with its protections and its limits; a dependency audit of the chosen crates finds no C code; a captured session does not decrypt after the application restarts. | not mapped in its tasks.md |

## WASM-014-backend-contract-and-evolution

**11 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A frontend built against contract 1.0 connecting to a backend at 1.2 is accepted; to a backend whose major version differs is refused with an update message; one older than the protocol window is refused with an update message, not a decoding error. | not mapped in its tasks.md |
| AC2 | `unaudited` | Each compatible change of R4 builds and an old frontend still works against it (a test runs the old client against the new backend); each incompatible change fails the release Build naming program, item and rule. | not mapped in its tasks.md |
| AC3 | `unaudited` | A deprecated program works and the frontend raises the warning event. | not mapped in its tasks.md |
| AC4 | `unaudited` | Each failure class is produced by a test (a dropped connection, a bad token, a refused role, a bad payload, an unknown program, a version refusal, an oversized frame, an error raised by the program, a crash) and the program reads the class and code; an unknown class from a newer backend is read as *server*. | not mapped in its tasks.md |
| AC5 | `unaudited` | A business failure raised in a backend program arrives in `ON EXCEPTION` with its code and data. | not mapped in its tasks.md |
| AC6 | `unaudited` | A dropped connection during a call to a program declared safe is repeated once and succeeds; during one not declared safe it fails and says whether the call may have been received; a repeat with the idempotency key returns the stored result without running twice. | not mapped in its tasks.md |
| AC7 | `unaudited` | Interleaved calls on one connection keep their order for dependent programs and may overlap for independent ones. | not mapped in its tasks.md |
| AC8 | `unaudited` | A long-running call reports progress and cancels on request; a stream delivers events, survives a reconnection and costs nothing after unsubscribing. | not mapped in its tasks.md |
| AC9 | `unaudited` | A file larger than a single frame uploads in pieces, resumes after a forced disconnection, and arrives with a matching checksum; a name with `../` is refused; an expired download address is refused. | not mapped in its tasks.md |
| AC10 | `unaudited` | All the above run against a backend and a frontend in a test, in both directions of version skew the windows allow. | not mapped in its tasks.md |
| AC11 | `unaudited` | The handshake records the key exchange; a frontend built against a contract version without it is refused with the reason; a captured handshake from one start of the application does not decrypt a session of the next start. | not mapped in its tasks.md |

## WASM-015-testing-agent-tooling-and-knowledge

**13 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | One command runs the NC subset on the generated module through the module runner and prints two numbers per module with the excluded programs and their reasons. | not mapped in its tasks.md |
| AC2 | `unaudited` | The browser runner builds, serves and drives a hello-world bundle on a headless engine, reads the DOM and a computed style and returns a picture, on a machine with no separately installed browser (or with the requirement stated and checked). | not mapped in its tasks.md |
| AC3 | `unaudited` | The backend runner starts a backend, signs a test user in against the local issuer, makes a call and a version-skew call, and tears down cleanly. | not mapped in its tasks.md |
| AC4 | `unaudited` | The web property round-trip tests pass for every class A and B control; the list of uncovered controls with a decision each exists. | not mapped in its tasks.md |
| AC5 | `unaudited` | The reference form's desktop and browser renderings agree within the plan's tolerance, with only the documented differences. | not mapped in its tasks.md |
| AC6 | `unaudited` | The ledger shows the generated module's NC, IF and IC rows; a regression run is one command. | not mapped in its tasks.md |
| AC7 | `unaudited` | The golden files for a web form's artefacts exist and regenerate with the golden switch. | not mapped in its tasks.md |
| AC8 | `unaudited` | Through the MCP tools on a web form: `create_project` with a kind, `check` (an error for a class D control and for a file verb), `build` (a bundle), `render_form` (a picture) and `run_form` (a click and a property read) all work and report the engine used. | not mapped in its tasks.md |
| AC9 | `unaudited` | The IDE-served and the stdio server give the same answers for the same web project. | not mapped in its tasks.md |
| AC10 | `unaudited` | The plugin's rules include the web rules; the reviewer flags a web form containing a `SqlDatabase`; the new skills exist. | not mapped in its tasks.md |
| AC11 | `unaudited` | `prebuilt_chunked_kb_matches_the_published_documentation` is green with the web knowledge; a Grace run asked for a `SqlDatabase` on a web form refuses or reroutes it through the backend. | not mapped in its tasks.md |
| AC12 | `unaudited` | The example Web application and Backend build and run in the test suite and contain no secret. | not mapped in its tasks.md |
| AC13 | `unaudited` | The reference form with layers gives the same stacking, visibility, hit testing and tab order on the desktop and in the browser, except the differences WASM-004 R17 and R20 record (R17). | not mapped in its tasks.md |

## WASM-016-prebuilt-player

**14 open.** Not audited against the code, one criterion at a time. Blocked part: the operator's manual look at the Leptos Surface (T22) and the finalize after it (T30).

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | On a machine with no Rust toolchain, a Web project builds to a bundle that contains exactly the player (plain and threaded) and the program module (multi-memory and single-memory variants), the host page, form descriptions, stylesheet and assets, and runs in a headless browser. | T28 (T14, T18 feed it) |
| AC2 | `unaudited` | A form with one control of every class A and B type is drawn by the player from its description, with the properties set; the DOM shows each, and the property round-trip of WASM-004 R11 passes. | T19 (description round trip), T17 (every control drawn, property round trip) |
| AC3 | `unaudited` | A click on a bound button reaches the program module's handler; a property written by the handler appears in the DOM; a method call reaches the control; `DISPLAY` reaches the console. | T1, T24 |
| AC4 | `unaudited` | A handler that makes a remote call stops, the player performs it against a test backend with the token attached, and the handler resumes with the answer; two events raised meanwhile are handled afterwards in order. | T1, T5, T24 |
| AC5 | `unaudited` | With the backend stopped the player retries three times (10 s, 20 s, 30 s — WASM-003 R25) behind the modal of R20, then raises the exception; the program sees `ON EXCEPTION` for the call in flight, and not for calls made after a successful reconnection. | T5, T24 |
| AC6 | `unaudited` | A bundle whose player lacks a host-API entry the module needs fails the Build; a module started against such a player refuses to start with a message. | T8, T25 |
| AC7 | `unaudited` | The freshness test fails when the player's source changes without the shipped player being rebuilt, and passes when it is rebuilt. | T11, T18, T25 (the archive) |
| AC8 | `unaudited` | The Build reports the player's size; a form with no chart, map or Viewer does not load their parts (a test inspects what was fetched). | T12, T18, T26, T27 |
| AC9 | `unaudited` | The reference form and the lifecycle, effect and event-order checks give the same results on the player and on the desktop form host, except for the documented differences. | T17 |
| AC10 | `unaudited` | A source search finds no HTML-string-inserting API in the player, the Viewer included; the player runs under the generated CSP; a connection to an address outside the configured backend is refused. | T4, T23, T28 |
| AC11 | `unaudited` | In an open Preview, replacing the module, a form description and the stylesheet each updates the page without a reload of the player. | T15 |
| AC12 | `unaudited` | A form with layers opened twice in a row starts both times with every layer hidden, and no layer state survives the release of the instance (R19). | T21 |
| AC13 | `unaudited` | With the backend stopped during a session, the modal appears within one frame of the break, covers the application, ignores clicks and keys sent to the form behind it (a text box behind it does not change), animates, and its background samples red at the left edge, then dark red, crimson and black at the right edge, with whit… | T29b (`e2e_chrome`, *backend lost*) |
| AC14 | `unaudited` | With the backend down for good, the modal shows the failure caption after the third attempt (10 s, 20 s and 30 s, measured) and the application's handler for the player's event runs; both captions appear in the language of the host page. | T29b (`e2e_chrome`, *backend down for good*) |

## WASM-017-module-architecture-and-js-boundary

**15 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A three-form project builds one program module; it is fetched once, at start, with streaming compilation; a heavy player part (a chart) is fetched only when a form using it is first shown. | T11 |
| AC2 | `unaudited` | Opening and closing a form 100 times leaves the live instance count and the module's used regions at their starting values; creating more instances than `Instances` raises the run-time error naming the form. | T2, T3, T11 (instance exhaustion also WASM-010 T9) |
| AC3 | `unaudited` | A form kept resident by `PreservePreviousForm` keeps its region; destroying it releases the region; an unmount terminates the Workers and drops the module. | T11 (Rayon pool limit stated, Q-17c) |
| AC4 | `unaudited` | A called program declared `INITIAL` starts fresh on every call; a plain called program keeps its state across calls until `CANCEL`, and after `CANCEL` starts fresh. | T10 |
| AC5 | `unaudited` | Generated modules contain no `memory.grow`; a program whose data cannot be sized fails to compile with the item named. | T5 |
| AC6 | `unaudited` | The multi-memory variant has two memories with the arena in memory 1; the single-memory variant has one and no multi-memory instruction (a scan of its sections); the host API behaves identically on both. | T6, T11 |
| AC7 | `unaudited` | The iOS Mobile build uses only the single-memory variant by default; a Web bundle whose matrix includes Safari carries both variants and loads the right one on Chrome (multi-memory) and on a browser that lacks it (single); "multi-memory only" is refused by the Build when the matrix includes Safari. | T7 (loader choice: existing "choose" scenario, T12) |
| AC8 | `unaudited` | A handler that writes 1000 properties makes at most the stated number of host calls, and a read after a write sees the write. | T11 |
| AC9 | `unaudited` | Text with multi-byte characters crosses the boundary intact in both directions; no JavaScript string is built in a loop (a trace counts conversions). | T8, T11 |
| AC10 | `unaudited` | A player call that grows the player's memory does not invalidate what the host reads next; no view is cached across calls (a source search of the loader and the test of R10). | T8, T11 |
| AC11 | `unaudited` | Every object the player exposes to JavaScript is on the list and is disposed by `unmount()`; a forced panic shows the readable error with the reload action and raises the error event. | T9 |
| AC12 | `unaudited` | The Build prints both player variants' sizes before and after `wasm-opt`, the profile in force, the pinned tool versions and the nightly's date. | T14 |
| AC13 | `unaudited` | On an isolated page the threaded player and the Worker execution mode are chosen and a Rayon-backed operation uses more than one worker; on a non-isolated page the plain player and the main thread are chosen, results are identical, and a ten-million-iteration loop keeps every frame under the test's deadline. | T12 |
| AC14 | `unaudited` | Embedded in a non-isolated host page the application starts on the plain player and works; embedded in an isolated one it uses threads. | T13 |
| AC15 | `unaudited` | The headers for isolation are generated for each documented host; an image, font or tile from another origin loads under `credentialless` or with CORS/CORP as documented, and fails with a readable diagnostic otherwise. | T13 |

## WASM-018-canvas-and-graphics-objects

**14 open.** The implementing tasks are done and their tests exist, but nobody has ticked these boxes against the code, one criterion at a time.

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC1 | `unaudited` | A Canvas2D on a form shows what a handler draws (a line, a filled and a stroked path, a gradient, shaped text in each of the six languages, an image) on the desktop, in a browser and in the mobile web view; the three pictures are pixel-identical (CPU path). | T18, T25, T27, T30 |
| AC2 | `unaudited` | A handler that draws ten thousand shapes makes **one** host call to present (a count of host calls in the generated module), and `ToImage` after drawing returns what was drawn. | T7 (library), T17 (interpreter), T24 (module), T25 (player) |
| AC3 | `unaudited` | `MeasureText` equals the extent of `DrawText` for Latin, Japanese, Chinese and right-to-left text; wrapped text breaks at the same places on every platform. | T2 (library), T25, T27, T30 |
| AC4 | `unaudited` | A glTF model with animation loads into a Scene3D and renders with lights and shadows on a software adapter; the picture matches the golden image within the tolerance; `Pick` returns the node under a pixel. | T6 (Metal), T12, T18, T29 |
| AC5 | `unaudited` | On a machine with WebGPU, WebGL2 only, and neither, `Backend` reports each and a Canvas3D renders or shows the readable message. | T6, T18, T26, T29, T30 |
| AC6 | `unaudited` | A Physics2D world with stacked boxes settles to the same final positions on desktop, plain player and threaded player; a body bound to a sprite moves it. | T5 (library), T30 |
| AC7 | `unaudited` | An ImageBuffer resized, blurred and encoded gives the same bytes with one thread and with many; a 24-megapixel blur on the threaded player is faster than on the plain one (measured, reported) and does not freeze the page. | T3, T9 (library), T30 |
| AC8 | `unaudited` | A Plot of each chart kind renders into a Canvas2D, an ImageBuffer and SVG text; a COBOL table of 100,000 values reaches a series in one pass. | T4, T7 (library), T32 (from COBOL) |
| AC9 | `unaudited` | Handles are released with their object and with the form; opening and closing a form with a canvas, an image and a scene 100 times leaves memory at its baseline (WASM-017 AC2). | T7 (library), T17, T31 |
| AC10 | `unaudited` | Limits refuse an oversized canvas, image, model or display list with a readable error; a corrupt image and a corrupt model are rejected without a crash (the fuzz targets run within their budget). | T3, T6, T7 (library), T12, T17, T31 |
| AC11 | `unaudited` | The dependency-guard test over `cobolt-graphics` passes and fails when a denied package is added; no build script in its default tree compiles C or C++. | T8, T11, T28 |
| AC12 | `unaudited` | Keyboard, pointer, wheel, touch and gamepad input reach the program on the desktop and in the browser; `onFrame` fires at `TargetFps`, pauses when hidden and steps at a fixed rate in fixed-step mode. | T19, T28, T30 |
| AC13 | `unaudited` | The Form Designer shows the placeholder; Preview, Run Form, the compiled binary and the player show the live drawing; the interpreter and the generated module give the same display list for the same program. | T18, T20, T24 |
| AC14 | `unaudited` | The sample project builds and runs on Desktop and Web; the Web build loads the 3D engine only when the 3D form is shown. | T26, T32 |

## WASM-036-stylesheet-values-in-designer-and-properties

**1 open.** Not audited against the code, one criterion at a time. Blocked part: the operator's comparison of the designer's canvas with the emulator's screen (T23).

| AC | Class | What it says | Verified in |
|---|---|---|---|
| AC16 | `blocked-operator` | (the operator's report) — In a copy of `examples/PowerMobileHello`, a form sheet colours the buttons and changes the label's font. The designer's canvas and the emulator's screen agree on those colours and that font. Both screenshots are kept as evidence. | T23 |

