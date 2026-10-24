<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Preview and Run in the browser and on the device; the web view stays headless

- **Status:** in progress. T1–T10 were done against the plan of 2026-10-20 and stay checked with their evidence (results at the bottom).
  The open tasks T14–T29 of 2026-10-22 were never implemented and are **replaced** by T30–T49, written against the plan of **2026-10-23**
  (spec §0, R12–R16, AC11–AC15; plan C10–C19). T11 stays, re-worded for the browser and the emulator.
- **Plan:** ./plan.md   **Date:** 2026-10-23   **Line:** 2.x.x (`features-2x`), a **feature**. Code read at **2.0.49**
  (`crates/cobolt-ide/src/version.rs:22`).

`[x]` was done and verified; `[ ]` is still to do. The first plan's commits (worktree branch `worktree-agent-a97ba65c65571c2df`):
`2eded9b0` (seam, server, presets, protocol), `d63c6faf` (helper process, settings section), `93b9124e` (controller, controls window,
wiring), `d97f4db6` (plugin rules and skills, System KB, regenerated store); merged into `features-2x` at 2.0.20.

`PT` is `crates/cobolt-project-tools`, `WP` is `crates/cobolt-web-preview`, `IDE` is `crates/cobolt-ide`, `CC` is `crates/cobolt-compiler`.

**Open questions — working assumptions, not decisions (plan §9):** Q1 → a separate process for the headless helper (D1; moot for the
interactive Preview; WASM-INDEX D7 still open). Q3 *not answered* → the real backend run locally by `BackendRun`, no stub (D8). Q5 *answered
"as short as possible", to confirm* → 500 ms for a stylesheet change, 500 ms plus the build's own time for a description or module change (D6);
every test **prints** the measured number and none asserts one the operator has not given. Q6 *not answered* → Preview and Run are two
actions; Run starts the backend first (D7). **Decided:** Q2 (wry + tao, D2), Q4 (operator 2026-10-22: a development token may be used, D4),
Q7 (at `/plan`, D11/C15 — `preview_browser` in `[web]` and `[mobile]`, a closed list or a confirmed path; the operator may overrule).
**Operator rulings applied (2026-10-23, spec §0):** browser for Web, emulator/simulator for Mobile with the browser fallback at a phone width,
the helper headless only, no presets in the IDE.

**Dependencies, not done here:** WASM-012's `mobile_toolchain` / `mobile_devices` (`ls CC/src` has neither today) for AC13; the player's
intake of `backend.dev_token` (today only `Client::set_token`, `crates/cobolt-web-player/src/net.rs:210`, fed by the Authentication
control) for R6 calls to authenticate — WASM-016 / WASM-009.

**Rules every task keeps** (CLAUDE.md, CONVENTIONS.md, `specs/steering/*.md`): every new widget or note text is a `Tr` key in **six
languages** in the same task that first shows it; page scripts and the phone page are **Rust string constants** (PRIME DIRECTIVE — no `.js`,
`.html` or other non-Rust file is added); the Preview **never writes** into the project's sources or `dist/web` (user code is sacred);
`README.md` and `docs/` are **never touched** on 2.x.x; no "cobolt" in user-facing text; no interpreter change, so interpreter/binary parity is
unaffected; no generated `.cbl` changes, so the banner/regenerate contract is unaffected. Each task leaves the tree green; helper-dependent
tests skip with a printed reason when the helper or a display is absent (plan §5, §7).

## Done (plan of 2026-10-20)

- [x] **T1 — The build/serve seam** (R1, R10)
  - Files: `PT/src/web/mod.rs`
  - Done: `WebBuildBackend` (build, bundle_dir, serve), `WebBuildRequest::from_manifest`, `BuildReport {bundle_dir, size, startup_ms, warnings, duration_ms}`, `WebBuildError {NotYetLinked, Refused, Failed}`, `link` / `unlink` / `backend` (`mod.rs:279`, `:289`), `NotYetLinked`.
  - Verify: `cargo test -p cobolt-project-tools --lib web::tests`.

- [x] **T2 — The development server** (R8)
  - Files: `PT/src/web/devserver.rs`
  - Done: loopback only, Host-header check (`host_is_ours`, `:440`), MIME table (`.wasm` = `application/wasm`), COOP/COEP from the project's isolation setting, `/__prc/version` (`VERSION_PATH`, `:44`) + `/__prc/livereload.js` (`RELOAD_SCRIPT_PATH`, `:46`), 15 spellings of a path outside the bundle refused, symlinks that leave the bundle not followed, GET/HEAD only (405 otherwise, `:525`).
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver` (prints the request timings).

- [x] **T3 — Presets** (R4)
  - Files: `PT/src/web/presets.rs` — 7 presets × 2 orientations (`PRESETS`, `:67`), pixel ratios 1/1.5/2/3, safe-area insets that turn with the screen.
  - Under the 2026-10-23 plan the IDE offers no presets (C10); `PRESETS` stays as the source of the headless viewport (T40) and of the phone width (T39).

- [x] **T4 — The helper process and its protocol** (R2, R5, R8)
  - Files: `WP/`, `PT/src/web/preview.rs`
  - Done: JSON lines pinned from both sides; `open` carries the emulation; `emulate` is one reload; console / error capture; navigation and new-window policy (`may_navigate`, `WP/src/main.rs:95`); the system-browser fall-back (`open_in_system_browser`, `preview.rs:172`); `PRC_WEB_PREVIEW` and `PRC_PREVIEW_NO_BROWSER` (`NO_BROWSER_ENV`, `preview.rs:47`).
  - Verify: `cargo test -p cobolt-web-preview` (5) and `cargo test -p cobolt-project-tools --lib web::preview` (5).
  - The visible window (`WindowBuilder` … `with_devtools(true)`, `WP/src/main.rs:263-283`) and the `size` / `devtools` commands (`:314`, `:337`) are removed by **T40** (C14).

- [x] **T5 — The controller** (R1, R3, R4, R7)
  - Files: `IDE/src/panels/web_preview.rs`
  - Done: build job, serve, session, `SourceWatcher`, rebuild + `notify_changed`, `Controls::changes_from`, `Note`s rendered from `Tr`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` (13).
  - `Controls` and the `PreviewSession` of the interactive Preview are removed by **T35** (C10).

- [x] **T6 — The controls window** (R4, AC3)
  - Done: 420 x 560 in six languages, one size; buttons and checkboxes in fixed boxes; the log takes what is left.
  - Verify: `the_controls_window_keeps_its_size`.
  - The window itself (`WebPreview::show`, `web_preview.rs:887`) is removed by **T35**; AC3 no longer applies (spec §5).

- [x] **T7 — Wiring** (R10, AC9)
  - Files: `IDE/src/app.rs` — designer Preview (toggle, `toggle_web_preview` `:2736`), Run (browser, `do_run` `:2823`), Build (bundle), Stop and *open another project* end it; File ▸ Build label by kind.

- [x] **T8 — Strings** — `pk_build_*` (9), `pk_prev_*` (25), `pk_ps_*` (7) in six languages.

- [x] **T9 — Agent surface** — `build` for a Web project returns the `BuildReport`; `list_files` shows `web_assets`; the plugin rules and skills mention the Preview (see WASM-001 T11).

- [x] **T10 — Final sweep** — results below.

## Replaced by this revision

- **T12, T13** (2026-10-20) and **T14–T29** (2026-10-22): never implemented; replaced by T30–T49. Specifically dropped with the plan
  (§0.3): the server-side `ChangeSet` and `/__prc/changes` (C17 — in-place update is the host page's `app.update`,
  `crates/cobolt-web-bundle/src/hostpage.rs:872`), the in-process `cobolt_server::dev::DevServer` in the IDE and its `cobolt-server`
  dependency (C18 — the backend is `BackendRun`, `IDE/src/backend_run.rs`), and the presets/controls work of the old T24/T28 (C10).

## To do (plan of 2026-10-23), in execution order

### The project setting and the launcher

- [ ] **T30 — `preview_browser` in `[web]` and `[mobile]`** (R12, Q7; plan §2.6, D11, C15)
  - Files: `CC/src/project_kind.rs` (`WebSettings`, `:363`; `MobileSettings`, `:446`).
  - Do: `pub preview_browser: String` on both, `#[serde(default, skip_serializing_if = "String::is_empty")]`. Empty = system default; a
    known id (`chrome`, `chromium`, `edge`, `firefox`, `safari`, `brave`); anything else = an application path. No reader yet.
  - Verify: `cargo test -p cobolt-compiler --lib project_kind --no-fail-fast` — an existing Web and an existing Mobile `cobolt.toml` (no key)
    load and save **byte-for-byte unchanged**; a set value (an id and a path) round-trips; the key is absent from the saved text when empty.

- [ ] **T31 — The launcher: `open_in_browser`, `BrowserChoice`, `KnownBrowser`** (R12, R8; plan §2.1)
  - Files: `PT/src/web/preview.rs` (beside `open_in_system_browser`, `:172`, and `is_preview_url`, `:200`).
  - Do: `BrowserChoice { System, Named(KnownBrowser), Path(PathBuf) }` parsed from the T30 string; `KnownBrowser` a closed list resolved
    per OS (macOS `open -a "<App>" <url>`; Windows the App Paths executable; Linux the first usual command on `PATH`); Safari only on macOS.
    A pure `launch_plan(url, &choice) -> Result<std::process::Command-shaped plan, String>` the tests can read; `open_in_browser` runs it.
    The `is_preview_url` guard first; `PRC_PREVIEW_NO_BROWSER` (`NO_BROWSER_ENV`, `:47`) makes it decide without starting anything.
    Std only, no new dependency. A browser not found is an error **naming it**; the caller falls back to `System` (T35).
  - Verify: `cargo test -p cobolt-project-tools --lib web::preview --no-fail-fast` — each known id resolves on the current OS to the
    printed program and arguments; an unknown id parses as a path; a non-loopback or non-`http` URL is refused for every choice; Safari on a
    non-macOS target is "not available"; with `NO_BROWSER_ENV` nothing is spawned.

### The served page and the dev server

- [ ] **T32 — `POST /__prc/log` and `PageEvent`** (R13, R5; plan §2.2, D13)
  - Files: `PT/src/web/devserver.rs` (the method check at `:525`; `ServerHandle`, `:106`).
  - Do: the only non-`GET`/`HEAD` route. Refused (and counted): wrong Host (`host_is_ours`), `Origin` not the server's own, body > 16 KiB,
    `Content-Type` not `application/json`, more than 40 events in a request or 200 per second, an event outside
    `console | error | cobol-error | updated`. Accepted events parsed into a typed `PageEvent` (never kept as raw bytes) and queued for
    `ServerHandle::take_page_events()`. Every other method on every other path still answers 405 (the existing test at `:801` stays).
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver --no-fail-fast` — each refusal case answered and counted, printed;
    a valid batch of the four kinds comes out of `take_page_events` in order with its `form`; the 405 test unchanged.

- [ ] **T33 — The Preview script** (R13, R5, R3; plan §2.2, D6)
  - Files: `PT/src/web/devserver.rs` (`reload_script`, `:488`; `with_reload_script`, `:502`), `WP/src/main.rs` (`INIT_SCRIPT`, `:103`).
  - Do: `reload_script` grows into one Rust constant that (a) wraps `console.*`, `window.onerror`, `unhandledrejection`, sending each event
    over `window.ipc` when the page is inside the helper and `fetch('/__prc/log', POST)` otherwise — so the helper's own capture in
    `INIT_SCRIPT` (`:104`) is dropped and both paths say the same thing (the plan's "one constant", with no new crate dependency for `WP`);
    (b) after each settled run reads `app.player.errors()` (`crates/cobolt-web-player/src/api.rs:920`) and sends the new entries as
    `cobol-error {form, unit, line, code, message}`; (c) times each `app.update` with `performance.now()` and sends `updated {how, ms}`;
    (d) tags every event with `form` — the route `#/form` in an SPA, `pr-form` in a standalone window; (e) shows a "the Preview has stopped"
    banner after three failed polls in a row. The poll drops from 400 ms (`setTimeout(poll,400)`, `:498`) to 150 ms (D6). Page strings come
    from data the IDE hands the server in its language (T35), inserted with `textContent` only.
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver --no-fail-fast` — the script names `app.update`, `errors()`, `/__prc/log`,
    `window.ipc`, `performance.now`, `150`; contains no `innerHTML` / `insertAdjacentHTML` / `eval`; the existing `app.update(c)` /
    `a==='reload'` assertion (`:840`) kept; `cargo test -p cobolt-web-preview --no-fail-fast` — the init-script test (`main.rs:451`) updated to
    what `INIT_SCRIPT` still carries. The page run itself is proved headless in T47.

- [ ] **T34 — A failed build shows its errors over the last good page** (R7, AC5; plan §1 R7 row)
  - Files: `PT/src/web/devserver.rs` (`ServerHandle::set_build_errors`; `"errors":[…]` in `/__prc/version`; the overlay in the T33 script).
  - Do: a dismissable overlay built with `textContent` (WASM-INDEX D14); the next good build clears it; `dist/web` untouched by the failure
    (the generator's atomic replace).
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver --no-fail-fast` — `set_build_errors` then `/__prc/version` carries them,
    an empty set removes the key; the overlay code uses `textContent` only. AC5 end to end is T47.

### The IDE: browser, notes, setting, backend

- [ ] **T35 — Preview and Run open the browser; the controls window goes** (R12, R16, R10; AC11, AC9; plan §1 R12 row, C10)
  - Files: `IDE/src/panels/web_preview.rs` (`Target`, `:478`; `Live`, `:530`; `WebPreview`, `:540`; `start`, `:607`; `serve`, `:756`;
    `fall_back_to_the_browser`, `:827`; `apply_controls` / `reload` / `devtools` / `show` / `body`, `:846`–`:922`; `Controls`, `:245`),
    `IDE/src/app.rs` (`web_launch`, `toggle_web_preview` `:2736`, `show_web_preview`, `do_run` `:2823`), `IDE/src/i18n.rs`,
    `PT/src/web/preview.rs` (`Mode::WebView`, `:217`, and the visible spawn of `PreviewSession::start`, `:235`).
  - Do: `Target::WebView` and `Controls` removed; `serve` serves, hands the server the page strings (T33/T34) in the IDE's language, then
    calls the T31 launcher with the project's `preview_browser`; a named browser that fails → `Note::BrowserFailed { name, why }` and the
    system browser. **No `PreviewSession` is started for an interactive Preview** — `Launch::helper` is no longer read there. A Preview
    already serving is **reused** (Preview again re-opens the same address). `WATCH_EVERY` (`:49`) 400 → 150 ms with one quiet look (D6).
    `show` keeps only what app.rs needs (no window). The `pk_prev_*`/`pk_ps_*` keys whose only reader was the controls window are removed
    from all six languages; new keys (`BrowserFailed`, the page strings) added in six. Every branch stays behind `is_web_project()`
    (`app.rs:2687`).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` — with `PRC_PREVIEW_NO_BROWSER` and a stand-in backend:
    no setting → the system browser chosen; `firefox` → that one; a missing one → system browser + `BrowserFailed`; the Preview spawns **no
    helper process** (asserted, AC15's "no visible helper window"); a second Preview reuses the URL; `every_note_reads_in_all_six_languages_without_a_gap`
    and `this_file_has_no_literal_text_for_a_widget` green; `the_controls_window_keeps_its_size` and the preset tests deleted with the code
    they tested (stated in the commit). `cargo test -p cobolt-project-tools --lib web::preview --no-fail-fast` green.

- [ ] **T36 — Page events become Output lines** (R13, R5, R3; AC12, AC4, AC2; plan §1 R13/R5 rows)
  - Files: `IDE/src/panels/web_preview.rs` (`poll_events`, `:789`; `Note`, `:65`), `IDE/src/i18n.rs`.
  - Do: each poll drains `take_page_events` (T32) into `Note::Console { level, text, form }`, `Note::PageError { …, form }`,
    `Note::CobolError { form, unit, line, code, message }` (line `form: message (unit:line)`), `Note::Updated { ms, how }` (beside the rebuild
    time printed at `:750`), `Note::PreviewStopped`; the 40-per-poll cap (`CHATTER_PER_POLL`, `:59`) and the 300-note log (`MAX_NOTES`, `:55`)
    kept. One Output line per `how` in six languages.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` — events POSTed to a served stand-in bundle become Output
    lines containing the form name, the COBOL unit and line for `cobol-error`, `how` and `ms` for `updated`; a flood stays at 40 per poll;
    `a_change_is_reported_once_the_folders_are_still` (`:1357`) updated to the 150 ms look and prints it.

- [ ] **T37 — The browser row and the path confirmation** (R12, Q7; plan §2.6)
  - Files: `IDE/src/panels/web_settings.rs` (`show`, `:286`; `row`, `:57`), `IDE/src/app.rs`, `IDE/src/i18n.rs`, a small machine-local
    store beside `debug_settings.toml` under `crate::llm::base_dir()` (the pattern of `IDE/src/debug_settings.rs:90`), **not** in the project.
  - Do: one row: *System default*, the known browsers present on this machine, *Other…* with a path field. The first Preview with a path
    asks once (*Start "<path>" to show the Preview?*), the answer kept per project folder + path in the machine-local store; unconfirmed or
    refused → `Note::BrowserNotConfirmed` and the system browser. The question is a fixed-size modal (the self-resizing golden rule).
    `preview_browser` is written only when the developer sets it.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_settings --no-fail-fast` and `… web_preview` — an unconfirmed path is **not
    started** (the launch plan is never run) and says so; a confirmed one is used next time; a project copied to another folder asks again;
    the settings row and the modal read in six languages without a gap and keep their rectangle over 120 frames in each.

- [ ] **T38 — The backend during Preview and Run** (R6; plan §2.5, D4, D7, D8)
  - Files: `IDE/src/backend_run.rs` (`BackendEvent`, `:26`; `poll`, `:93`), `IDE/src/app.rs` (`start_backend` `:2772`, `poll_backend` —
    `BackendEvent::Line(line) => self.output.push_line(line)` prints the token in clear today; `do_run` `:2823`), `PT/src/web/devserver.rs`
    (the per-run `config.json`), `IDE/src/panels/web_preview.rs`.
  - Do: `BackendRun::token()` = the line after `development token …` (`crates/cobolt-cli/src/main.rs:587-588`); the Output panel shows
    `<development token>` instead (as the test at `backend_run.rs:199` prints it). On serve, `config.json` is **rewritten in the response
    only**: the backend's `ws://127.0.0.1:…`, `unsafe_plain_ws: true`, `backend.dev_token`. Run starts the project's backend first when it has
    one (D7, working assumption); Preview offers it; none running → `Note::BackendOff` (exists) — never silent. Until the player takes the
    token (dependency above), calls answer the authentication error frame and the Output line says why.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide backend_run --no-fail-fast` and `… web_preview` with a fixture `backend/` — the
    served `config.json` names the backend and carries a token; the Output panel never contains the token's text; after the run a search of
    `dist/web` finds no token and `config.json` on disk is byte-identical to before.

### Mobile

- [ ] **T39 — Mobile: the device first, the browser at a phone width otherwise** (R14, R15; AC13, AC14; plan §2.3, D12)
  - Files: `IDE/src/panels/web_preview.rs` (the `DeviceRunner` trait and its "no device tools" implementation; the Mobile branch),
    `PT/src/web/devserver.rs` (`/__prc/phone`, written on the fly with the bundle's isolation headers), `IDE/src/app.rs` (the status line),
    `IDE/src/i18n.rs`.
  - Do: `DeviceRunner::{detect, list, run(dev_url), log}` — the seam WASM-012's `mobile_toolchain` / `mobile_devices` will implement; until
    then the only implementation answers "no device tools", so **every** Mobile Preview takes the fallback. `/__prc/phone`: a same-origin
    `<iframe src="/">` at the CSS size of `[mobile] device` + `orientation` (`MobileSettings`, `project_kind.rs:448`, `:450`; `PRESETS`),
    the not-the-device banner above it; opened through the T31 launcher with `[mobile] preview_browser`. `Note::NotTheDevice { platform,
    install }` and the status line "Preview in the browser, not on the device", six languages.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` — a Mobile project with the no-tools runner opens
    `/__prc/phone` (decision read with `PRC_PREVIEW_NO_BROWSER`), the note names the missing tool in six languages (**AC14**, the IDE half);
    a fake `DeviceRunner` with a device receives the dev address and its log lines reach the Output notes, calls printed in order (**AC13**,
    the hand-over only — the real device run **waits for WASM-012** and is not claimed). `cargo test -p cobolt-project-tools --lib
    web::devserver` — the phone page carries the bundle's COOP/COEP and the preset's width. The frame's `innerWidth` and
    `crossOriginIsolated` read headless are in T47.

### The headless helper

- [ ] **T40 — The helper is headless only; the protocol of plan §3** (R16, R9, R2; AC15; plan §2.4, §3, C14)
  - Files: `WP/src/main.rs` (`run`, `:234`; the `open` arm `:260`; `size` `:314`; `devtools` `:337`; the `lines` module `:38`; the header
    comment `:17-18`), `WP/Cargo.toml` (the description at `:9`), `PT/src/web/preview.rs` (`Command`, `:52`; `Event`, `:99`; `Mode`;
    `PreviewSession`, kept for the headless runner only).
  - Do: the window is always built `with_visible(false)` **and** placed off every screen (the hidden-window risk, plan §5); `size`,
    `devtools` and `title` removed — the helper answers `failed` to `size` / `devtools`; `open` takes `width, height, dpr, dark, locale`;
    `with_devtools(true)` (`:283`) removed; `may_navigate` kept. Events `console` / `error` gain `form`; `cobol-error` added (the same JSON as
    `/__prc/log`, parsed by one `PageEvent` type). Tolerant parsing: an unknown command is ignored by the helper, an unknown event or field by
    the IDE.
  - Verify: `cargo test -p cobolt-web-preview --no-fail-fast` and `cargo test -p cobolt-project-tools --lib web::preview --no-fail-fast` —
    each line pinned byte-for-byte from both sides; an old `console` line (no `form`) still parses; `size`/`devtools` answer `failed`;
    `a_session_relays_events_and_notices_a_helper_that_died` (`preview.rs:468`) green (R2 for the helper, the remaining half of AC6); the
    helper reports its window not visible.

- [ ] **T41 — `render` and `script`** (R9; AC8; plan §2.4)
  - Files: `WP/src/main.rs`, `PT/src/web/preview.rs` (a headless session API over the same pipes).
  - Do: `render {path}` → `rendered {path, width, height}` through the engine's snapshot reached by wry's platform handles; `script {steps}`
    with the closed steps `click`, `type`, `wait`, `get` (+ WASM-015's read-only `text`, `style`, `rect`, `snapshot`, `focused`, `key`) →
    `scripted {values, text, errors}`, through the player's `dispatch` (`api.rs:960`), `type_into` (`:973`), `invoke` (`:951`), waiting on
    `settled` (`:1008`); **no `eval`**. An engine that cannot capture answers `failed` naming it — never a blank picture.
  - Verify: `cargo test -p cobolt-web-preview --no-fail-fast` on a served fixture page — the PNG's size equals the requested viewport × dpr;
    a script that clicks, types, waits and reads a property answers the values, text and errors; skips with a printed reason without a
    display, and **fails** instead of skipping when `PRC_REQUIRE_WEBVIEW=1` (the CI switch, plan §5).

- [ ] **T42 — The helper on Linux** (R9, R16; plan §2.4, D2)
  - Files: `WP/Cargo.toml` (the target gate at `:39` and the comment `:33-38`), `WP/src/main.rs` (the `cfg`s at `:145`, `:153`, `:190`,
    `:195`, `:204` — Linux gets the real `main`/`run`; the fallback stays for other systems), `THIRD_PARTY_NOTICES.md`.
  - Do: wry's WebKitGTK 4.1 backend; licences from `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview`
    recorded (wry/tao/webkit2gtk/gtk bindings MIT/Apache; the system WebKitGTK LGPL, dynamically linked); no `ring` / `aws-lc-rs`.
  - Verify: `cargo test -p cobolt-web-preview --no-fail-fast` green on macOS; the `cargo tree` output and each licence written into this
    task when done; `cargo tree … | grep -E "ring|aws-lc"` empty. The Linux build and tests run in CI (T45) — **not runnable on this machine**,
    stated as such.

- [ ] **T43 — `render_form` and `run_form` for a web form** (R9, R16; AC8, AC15, AC9; plan §2.4)
  - Files: `PT/src/tools/mod.rs` (`render_form` `:520`, `run_form` `:537`, `run_form_in` `:553`), `PT/src/content.rs` (`:449-453` says
    today that `render_form`/`run_form` "show the desktop engine's picture" for a web form and that "only the IDE's Preview shows the page";
    `:1201-1205` tells the agent the developer looks at "a device, an orientation, the language, the console" in the IDE — both rewritten:
    the Preview is the browser / emulator, the tools draw a web form with the system web view, headless), `PT/tests/tools.rs`.
  - Do: a `.wcfrm` / `.mcfrm` in a Web or Mobile project builds once through `web::backend()`, serves, runs T40/T41 headless; the answer is
    the desktop fields plus `"engine": "webkit" | "webview2" | "webkitgtk"`. No helper → an error saying so (a `Tr`-free tool message, the MCP
    surface is English), **never** a desktop picture of a web form. `.cfrm` keeps its path untouched.
  - Verify: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` — the web answers' key set equals the desktop one plus `engine`;
    5 renders of the reference form within per-channel ≤ 2 and ≤ 0.5 % of pixels, the difference printed (**AC8**); the desktop
    `render_form` / `run_form` cases unchanged (**AC9**); no visible window reported (**AC15**).

- [ ] **T44 — Layers start hidden** (R11, AC10; plan §1 R11 row)
  - Files: `PT/tests/tools.rs` (a layered `.wcfrm` fixture, run headless).
  - Do: nothing in the Preview's code — it holds no layer state; the form description carries no layer `Visible`
    (`cobolt_forms::model::Layer`).
  - Verify: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` — each layer's visibility read by `script` is `false` at open and
    after an `app.update` restart, `true` only after the program's `SET … Visible TO TRUE` handler runs.

### Shipping and the System KB

- [ ] **T45 — The helper ships; its runtime libraries are declared** (R9, R16; spec §6; plan §6)
  - Files: `.github/workflows/build-all-platforms.yml` (no `prc-web-preview` today; `.deb` `Depends:` at `:1016` names only
    `libasound2 | libasound2t64`; `.rpm` `Requires:` at `:1155` only `libasound.so.2()(64bit)`; the requirements line at `:1376`),
    `IDE/src/toolchain.rs` (`detect`, `:204`).
  - Do: build and package `prc-web-preview` beside the IDE on macOS, Windows, Linux; `.deb`/`.rpm` gain WebKitGTK 4.1, the Windows text names
    WebView2 — each stated as needed for **agent and test renders** only (follow the `installer-look` skill for the package text); the CI runs
    `cargo test -p cobolt-web-preview` with `PRC_REQUIRE_WEBVIEW=1` (Xvfb on Linux). `detect` reports a missing WebKitGTK/WebView2 as
    "render_form/run_form cannot draw web forms", never as a Preview or Build blocker; its text is a `Tr` key in six languages.
  - Verify: the packaging steps list `prc-web-preview` next to the IDE on all three systems (read from the workflow file; the run is the CI's);
    `cargo test -p cobolt-ide --bin cobolt-ide toolchain --no-fail-fast` green.

- [ ] **T46 — The System KB stays true** (steering: the System KB rule)
  - Files: `assets/knowledge/chunked.data`, only if T43's `content.rs` change turns the freshness test red.
  - Do: no control, property, method or event changes, so no compiler doc table changes.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` (`grace_host.rs:4780`); if
    red, `cargo run -p cobolt-ide --example build_chunked_kb` in the same change, then green.

## Acceptance evidence

- [ ] **T47 — AC11, AC12, AC5, AC14 driven headless on the real build** (R12, R13, R3, R5, R7, R15; plan §7)
  - Depends on: T33–T41. Uses the real Web Build (WASM-010's generator and the shipped player, C16) on a hello-world Web project and a Mobile
    one; the browser launch is decided with `PRC_PREVIEW_NO_BROWSER`, and the **served page** is driven by the headless helper.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` —
    **AC11:** the launcher's choice printed (default / named); a scripted click changes the label; no helper spawned by the Preview.
    **AC12:** a property, a class rule and the stylesheet each arrive as `updated` with `how` ≠ `reload` and no page reload (the player
    instance unchanged), `ms` printed per kind against D6's working 500 ms (Q5 *to confirm* — reported to the operator, not asserted); a
    `console.error` and a thrown error reach the Output notes with the form name; a COBOL run-time error with unit and line.
    **AC5:** a stand-in failing on the second build — the bundle is still the first, the overlay's text read back by `script`.
    **AC14:** `/__prc/phone`'s frame `innerWidth` equals the preset's CSS width and `crossOriginIsolated` is `true` inside it.
    The measured times are written into this file's results.

- [ ] **T48 — AC9 and AC15 on the whole IDE** (R10, R16)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`, every `test result` line read — the desktop Preview window
    (`show_preview_window`) and `ExternalFormRun::spawn` tests unchanged and green; no web action for a Desktop project; no interactive
    path spawns `prc-web-preview` (the T35 assertion). Failures that pre-date this spec are named, not hidden.

- [ ] **T11 — Once, by the operator** (R12, R14, R15; AC11, AC13, AC14 — the look)
  - Preview a Web project in the default and in a named browser; edit the stylesheet and watch it change; break the build and see the
    overlay; close the IDE and see the stopped banner; Preview a Mobile project with an emulator (after WASM-012) and without one. An agent
    never drives the IDE or a browser window (operator ruling, 2026-07-15); it uses the headless helper and the MCP tools.

## Finalize

- [ ] **T49 — Version, CHANGELOG, tests of the touched crates** (all)
  - Files: `IDE/src/version.rs` (`VERSION`, `:22`; `2.0.49` when this list was written — bump `z` **once** from whatever it is then),
    `CHANGELOG.md`.
  - Do: **one** entry above the previous `## [PowerRustCOBOL 2.0.x]` heading (`CHANGELOG.md:20` today), dated, marked *Written on
    `features-2x`*, saying what changed: Preview and Run in the browser (named or default), the browser setting, the device-first Mobile
    Preview with the phone-width fallback, page events and update times in the Output panel, the error overlay, the backend and its token
    during Preview, the headless-only helper with render/script and the web route of `render_form`/`run_form`, the helper on Linux and in
    the installers. **No documentation task:** `README.md` and `docs/` are never touched on 2.x.x; what 2.x.x says about itself stays in
    `specs/`. Check this file's boxes with their evidence.
  - Verify: `df -h .` first (a worktree sweep needs disk); then, each with `--no-fail-fast`, reading every `test result` line:
    `cargo test -p cobolt-compiler --no-fail-fast`, `cargo test -p cobolt-project-tools --no-fail-fast`,
    `cargo test -p cobolt-web-preview --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo fmt --check` on the
    files touched (the tree is not fmt-clean — format only what changed); no new warnings.

## Coverage of the acceptance criteria

| AC | Tasks whose verification covers it |
|---|---|
| AC1 — read through AC11 | T35, T47; T11 the look |
| AC2 — read through AC12 | T33, T36, T47 |
| AC3 — presets and the window's rectangle | **no longer applies** (spec §5); T6 (done) history only |
| AC4 — read through AC12 | T32, T33, T36, T47 |
| AC5 — a broken build keeps the last good page with the errors over it | T34, T47 |
| AC6 — killing the web view's process | **no longer applies** to the interactive Preview (spec §5); the helper half: T4 (done), T40 |
| AC7 — off-origin link | the browser's own behaviour (spec §5); the headless helper keeps `may_navigate`: T4 (done), T40 |
| AC8 — headless render and scripted run, same fields, stable image | T41, T43 |
| AC9 — desktop Preview and Run Form as before | T7 (done), T35, T43, T48 |
| AC10 — a layered form shows no layer until the program shows one | T44 |
| AC11 — web Preview in the default or the named browser, a click works, no IDE window | T30, T31, T35, T37, T47; T11 |
| AC12 — in-place update with times, console and errors with the form name | T32, T33, T36, T47 |
| AC13 — mobile Preview on an emulator, device log in the Output panel | T39 (hand-over by a fake) — **the real run waits for WASM-012**; T11 |
| AC14 — no emulator → the browser at a phone width, says it is not the device | T39, T47; T11 |
| AC15 — `render_form` / `run_form` from the headless helper, no visible helper window | T35, T40, T43, T48 |
| R6 (no AC) — the backend on request, never a silent failure | T38 |
| Q7 (no AC) — the browser setting | T30, T37 |

## Results (T10, 2026-10-20)

- `cargo test -p cobolt-project-tools --lib web::` — 29 of the lib's 100 tests, all passing; `web::preview` 5.
- `cargo test -p cobolt-web-preview` — 5 passed (the helper's `run` module is compiled on macOS, so the wry/tao calls are type-checked).
- `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — 13 passed. What they print: 120 note renderings (20 notes x 6 languages, none empty, none with a `{}` left); build -> serve -> web view in 337-499 ms with a stand-in generator and helper, a form edit rebuilt and republished (`/__prc/version` rose), the controls reached the helper as 5 lines of which 1 was `emulate`; the controls window held 420 x 560 points over 120 frames (6 languages x busy and empty), the log never under 136 points.
- Dev server (debug build, loopback): 200 requests for a 17-byte file in 114 ms (1744 req/s); a 3 MiB `.wasm` 10 times in 157 ms (191 MiB/s); a changed stylesheet moved the change count in 166 ms (watch interval 150 ms); 15 spellings of a path outside the bundle refused, none leaked.
- Whole IDE binary: 1866 passed, 3 failed — the same three that WASM-001 tasks.md names (not related).
- **Not run, and not claimed:** a real window (no agent drives one), the helper on Windows or Linux, AC1 and AC2 with a real generator, AC8.
