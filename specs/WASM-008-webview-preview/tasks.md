<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Preview and Run in the browser and on the device; the web view stays headless

- **Status:** in progress. **2026-11-01 (2.0.123):** T47, T48 and T49 done; only **T11** is open — the operator's own look at the Preview (and the Mobile emulator half, after WASM-012). **2026-11-01 (2.0.122):** T41–T46 done; stopped at T47 (an operator decision on the step vocabulary and the reload signal, see T47). T1–T10 were done against the plan of 2026-10-20 and stay checked with their evidence (results at the bottom).
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

- [x] **T30 — `preview_browser` in `[web]` and `[mobile]`** (R12, Q7; plan §2.6, D11, C15)
  - Files: `CC/src/project_kind.rs` (`WebSettings`, `:363`; `MobileSettings`, `:446`).
  - Do: `pub preview_browser: String` on both, `#[serde(default, skip_serializing_if = "String::is_empty")]`. Empty = system default; a
    known id (`chrome`, `chromium`, `edge`, `firefox`, `safari`, `brave`); anything else = an application path. No reader yet.
  - Verify: `cargo test -p cobolt-compiler --lib project_kind --no-fail-fast` — an existing Web and an existing Mobile `cobolt.toml` (no key)
    load and save **byte-for-byte unchanged**; a set value (an id and a path) round-trips; the key is absent from the saved text when empty.
  - Done (2.0.121): `preview_browser` on `WebSettings` and `MobileSettings` (and carried by `web::WebBuildRequest`'s Mobile→Web mapping); `the_preview_browser_is_optional_and_only_written_when_set_008` — an existing `[web]` (232 B) and `[mobile]` (117 B) table saved byte for byte, `firefox` and two paths round-trip; `project_kind` 19/19.

- [x] **T31 — The launcher: `open_in_browser`, `BrowserChoice`, `KnownBrowser`** (R12, R8; plan §2.1)
  - Files: `PT/src/web/preview.rs` (beside `open_in_system_browser`, `:172`, and `is_preview_url`, `:200`).
  - Do: `BrowserChoice { System, Named(KnownBrowser), Path(PathBuf) }` parsed from the T30 string; `KnownBrowser` a closed list resolved
    per OS (macOS `open -a "<App>" <url>`; Windows the App Paths executable; Linux the first usual command on `PATH`); Safari only on macOS.
    A pure `launch_plan(url, &choice) -> Result<std::process::Command-shaped plan, String>` the tests can read; `open_in_browser` runs it.
    The `is_preview_url` guard first; `PRC_PREVIEW_NO_BROWSER` (`NO_BROWSER_ENV`, `:47`) makes it decide without starting anything.
    Std only, no new dependency. A browser not found is an error **naming it**; the caller falls back to `System` (T35).
  - Verify: `cargo test -p cobolt-project-tools --lib web::preview --no-fail-fast` — each known id resolves on the current OS to the
    printed program and arguments; an unknown id parses as a path; a non-loopback or non-`http` URL is refused for every choice; Safari on a
    non-macOS target is "not available"; with `NO_BROWSER_ENV` nothing is spawned.
  - Done (2.0.121): `KnownBrowser`, `BrowserChoice`, `LaunchPlan`, `launch_plan`, `open_in_browser` in `web/preview.rs`; on this Mac Chrome and Safari resolve to `open -a "<App>" <url>`, Chromium/Edge/Firefox/Brave answer "… was not found on this computer"; `web::preview` 6/6.

### The served page and the dev server

- [x] **T32 — `POST /__prc/log` and `PageEvent`** (R13, R5; plan §2.2, D13)
  - Files: `PT/src/web/devserver.rs` (the method check at `:525`; `ServerHandle`, `:106`).
  - Do: the only non-`GET`/`HEAD` route. Refused (and counted): wrong Host (`host_is_ours`), `Origin` not the server's own, body > 16 KiB,
    `Content-Type` not `application/json`, more than 40 events in a request or 200 per second, an event outside
    `console | error | cobol-error | updated`. Accepted events parsed into a typed `PageEvent` (never kept as raw bytes) and queued for
    `ServerHandle::take_page_events()`. Every other method on every other path still answers 405 (the existing test at `:801` stays).
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver --no-fail-fast` — each refusal case answered and counted, printed;
    a valid batch of the four kinds comes out of `take_page_events` in order with its `form`; the 405 test unchanged.
  - Done (2.0.121): `POST /__prc/log`, `PageEvent` (kebab-case `event` tag), `take_page_events`, `page_events_refused`; 10 refusal cases (403 ×4, 415, 413 ×2, 400 ×3) and a 429 burst counted; the route answers only while live reload is on; `web::devserver` 16/16 at the time.

- [x] **T33 — The Preview script** (R13, R5, R3; plan §2.2, D6)
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
  - Done (2.0.121): `PREVIEW_SCRIPT` (5 020 bytes served) with `PageStrings` as data; the helper's `INIT_SCRIPT` keeps only the emulation and `lines::from_page` forwards the four page events; `node --check` (outside the repository) parses it; `web::devserver` 18/18, `cobolt-web-preview` 5/5.

- [x] **T34 — A failed build shows its errors over the last good page** (R7, AC5; plan §1 R7 row)
  - Files: `PT/src/web/devserver.rs` (`ServerHandle::set_build_errors`; `"errors":[…]` in `/__prc/version`; the overlay in the T33 script).
  - Do: a dismissable overlay built with `textContent` (WASM-INDEX D14); the next good build clears it; `dist/web` untouched by the failure
    (the generator's atomic replace).
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver --no-fail-fast` — `set_build_errors` then `/__prc/version` carries them,
    an empty set removes the key; the overlay code uses `textContent` only. AC5 end to end is T47.
  - Done (2.0.121): `ServerHandle::set_build_errors`; `/__prc/version` carries `errors` while any; the overlay uses `textContent` only; `a_failed_build_s_errors_ride_on_the_version_until_a_good_one_008`.

### The IDE: browser, notes, setting, backend

- [x] **T35 — Preview and Run open the browser; the controls window goes** (R12, R16, R10; AC11, AC9; plan §1 R12 row, C10)
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
  - Done (2.0.121): `Target`, `Controls`, the controls window, `PreviewSession` in `Live`, and 24 `pk_prev_*`/`pk_ps_*` keys removed (with `the_controls_window_keeps_its_size`, the four controls tests, `nothing_is_drawn_while_the_window_is_closed` and `a_web_view_that_cannot_start_leaves_the_browser_not_nothing`, whose code is gone); 8 keys added in six languages; `preview_opens_the_projects_browser_reuses_its_address_and_starts_no_helper` — system / Firefox absent → `BrowserFailed` + system / Chrome present → Chrome / missing path → `BrowserFailed` / unconfirmed path → `BrowserNotConfirmed`, a second Preview reuses the address (no new build), the marking helper never ran; `web_preview` 15/15, `i18n` 21/21.

- [x] **T36 — Page events become Output lines** (R13, R5, R3; AC12, AC4, AC2; plan §1 R13/R5 rows)
  - Files: `IDE/src/panels/web_preview.rs` (`poll_events`, `:789`; `Note`, `:65`), `IDE/src/i18n.rs`.
  - Do: each poll drains `take_page_events` (T32) into `Note::Console { level, text, form }`, `Note::PageError { …, form }`,
    `Note::CobolError { form, unit, line, code, message }` (line `form: message (unit:line)`), `Note::Updated { ms, how }` (beside the rebuild
    time printed at `:750`), `Note::PreviewStopped`; the 40-per-poll cap (`CHATTER_PER_POLL`, `:59`) and the 300-note log (`MAX_NOTES`, `:55`)
    kept. One Output line per `how` in six languages.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` — events POSTed to a served stand-in bundle become Output
    lines containing the form name, the COBOL unit and line for `cobol-error`, `how` and `ms` for `updated`; a flood stays at 40 per poll;
    `a_change_is_reported_once_the_folders_are_still` (`:1357`) updated to the 150 ms look and prints it.
  - Done (2.0.121): `Note::Console`/`PageError` carry `form`; `Note::CobolError` (`form: COBOL run-time error code: message (unit:line)`), `Note::PageUpdated { how, ms }` with one line per `UpdateKind` (styles, program, forms, reload, unchanged), `Note::PreviewStopped`; 7 keys in six languages; `page_events_become_output_lines_with_the_form_008` — 5 posted kinds → 5 lines, a burst of 80 → 40; the watcher reports a save in 310 ms at the 150 ms look (was one 400 ms look plus a quiet one); a failed rebuild's lines reach `/__prc/version` and a good one clears them (`building_serving_opening_and_rebuilding_on_a_change`); `web_preview` 16/16.

- [x] **T37 — The browser row and the path confirmation** (R12, Q7; plan §2.6)
  - Files: `IDE/src/panels/web_settings.rs` (`show`, `:286`; `row`, `:57`), `IDE/src/app.rs`, `IDE/src/i18n.rs`, a small machine-local
    store beside `debug_settings.toml` under `crate::llm::base_dir()` (the pattern of `IDE/src/debug_settings.rs:90`), **not** in the project.
  - Do: one row: *System default*, the known browsers present on this machine, *Other…* with a path field. The first Preview with a path
    asks once (*Start "<path>" to show the Preview?*), the answer kept per project folder + path in the machine-local store; unconfirmed or
    refused → `Note::BrowserNotConfirmed` and the system browser. The question is a fixed-size modal (the self-resizing golden rule).
    `preview_browser` is written only when the developer sets it.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_settings --no-fail-fast` and `… web_preview` — an unconfirmed path is **not
    started** (the launch plan is never run) and says so; a confirmed one is used next time; a project copied to another folder asks again;
    the settings row and the modal read in six languages without a gap and keep their rectangle over 120 frames in each.
  - Done (2.0.121): one row (System default / the known browsers present here / Other… with a path) in `web_settings.rs` `browser_row`; `BrowserConfirmations` in `preview_browsers.toml` under `llm::base_dir()`, keyed by the canonical project folder + path; `show_browser_confirm` is a `fixed_dialog` of 460 px; `start_web` asks before an unconfirmed path, an answer is recorded and the Preview restarts; 8 keys in six languages. `a_browser_path_is_confirmed_once_per_project_folder_008` (unanswered → `None`, the launch never run → `BrowserNotConfirmed`; answered → used; a copied folder asks again) and `the_browser_row_and_the_confirmation_read_in_six_languages_and_keep_their_size_008` (708 frames, rectangles unchanged); `web_settings` 13/13, `web_preview` 16/16.

- [x] **T38 — The backend during Preview and Run** (R6; plan §2.5, D4, D7, D8)
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
  - Done (2.0.121): `BackendRun::token()` keeps the line after `development token …`, and the line handed on is `<development token>` (`TOKEN_SHOWN`), so neither the Output panel nor the failure tail can see it; `devserver::with_dev_backend` / `ServerHandle::set_dev_backend` rewrite `/config.json` in the response only (loopback `ws://` only: the address in `backend.origins`, `unsafe_plain_ws: true`, `backend.dev_token`); the Preview hands the pair over once both are printed, for the Backend it said Ready for, and clears it on Stop or a new start. The player reads no `dev_token` yet (grep: only cobolt-server and the CLI do), so `Note::TokenUnused` (Warn, six languages) says calls answer an authentication error until WASM-016 sends it. Run starts the Backend first (`start_with_backend`); a Preview without one says `BackendOff`. `the_token_is_kept_and_never_handed_on_as_a_line_008`, `the_ide_starts_the_backend_reads_its_log_and_stops_it` (real `rcrun`: token kept, no line carries it), `the_development_backend_rides_in_the_served_config_only_008` (devserver 19/19), and in `a_preview_starts_the_backend_waits_for_it_and_builds_the_page_for_where_it_listens` the served `config.json` carries the real token while the file on disk is byte-identical and no bundle file, line or note contains it; `backend_run` + `web_preview` 20/20 (1 ignored).

### Mobile

- [x] **T39 — Mobile: the device first, the browser at a phone width otherwise** (R14, R15; AC13, AC14; plan §2.3, D12)
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
  - Done (2.0.121): `DeviceRunner { detect, list, run(dev_url), log }` and `NoDeviceTools` (answers `adb` / `xcrun simctl` missing) in `web_preview.rs`; `Launch` gains `platform` and `devices` (`app.rs` `web_launch` passes `[mobile] platform` and `NoDeviceTools`). `open_on_device`: the first listed device runs the dev address (`Note::OnDevice`, its log read each poll as `Note::DeviceLog`, 40 a poll); otherwise `ServerHandle::set_phone` + `phone_url()` and `Note::NotTheDevice { platform, install, width, height }`, whose second line is the status line "Preview in the browser at W × H CSS pixels, not on the device"; 5 keys in six languages. `devserver::phone_page`: the banner as escaped text over a same-origin `<iframe src="/">` of the preset CSS size, 404 until set. `the_phone_page_frames_the_application_at_the_preset_width_isolated_008` (390 × 844, COOP `same-origin`, COEP `require-corp`; devserver 20/20); `a_mobile_preview_without_device_tools_opens_the_phone_page_008` (phone portrait → `/__prc/phone` at 390 × 844, tablet landscape → 1180 × 820, decided with `PRC_PREVIEW_NO_BROWSER`, the note in six languages names `adb`); `a_mobile_preview_hands_the_address_to_a_device_and_reads_its_log_008` (calls in order: `detect android`, `list android`, `run Pixel 8 http://127.0.0.1:…/`; no browser opened; the log lines are notes). AC13 is claimed for the hand-over only; the real device run waits for WASM-012. `web_preview` + `i18n` 39/39 (1 ignored).

### The headless helper

- [x] **T40 — The helper is headless only; the protocol of plan §3** (R16, R9, R2; AC15; plan §2.4, §3, C14)
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
  - Done (2.0.121): the helper builds its window `with_visible(false)`, at `(-32000, -32000)`, undecorated, and on macOS with `ActivationPolicy::Accessory` (no Dock icon); `with_devtools(true)` and `title` gone; `size` / `devtools` answer `failed` (`lines::removed`); an unknown command is ignored; `ready` carries `visible` from `Window::is_visible`. IDE side (`preview.rs`): `Command::Open { url, width, height, dpr, dark, locale }`, `Size` and `Devtools` removed; `Event::{Ready { visible }, Blocked, Failed, Closed, Page(PageEvent)}` — the page events parsed by the one `devserver::PageEvent` type, an unknown event or field ignored; `Mode` and the system-browser fallback removed from `PreviewSession` (headless only: a missing helper is an error naming it). Header comment and `Cargo.toml` description rewritten. Pinned both sides: `the_lines_the_ide_writes_are_the_helpers_contract` (and `size`/`devtools`/`title` no longer parse), `the_lines_the_helper_writes_are_understood` (an old `console` line without `form` reads, `ready` with and without `visible`), `the_events_are_the_ides_contract`; `a_session_relays_events_and_notices_a_helper_that_died` green. New `tests/headless.rs` runs the **built** helper on a loopback page: `{"event":"ready","visible":false}`, `size`/`devtools` refused, an unknown command ignored, `closed`, exit 0 (macOS, 0.65 s). `cobolt-web-preview` 5 + 1, `cobolt-project-tools --lib web::` 53/53.

- [x] **T41 — `render` and `script`** (R9; AC8; plan §2.4)
  - Files: `WP/src/main.rs`, `PT/src/web/preview.rs` (a headless session API over the same pipes).
  - Do: `render {path}` → `rendered {path, width, height}` through the engine's snapshot reached by wry's platform handles; `script {steps}`
    with the closed steps `click`, `type`, `wait`, `get` (+ WASM-015's read-only `text`, `style`, `rect`, `snapshot`, `focused`, `key`) →
    `scripted {values, text, errors}`, through the player's `dispatch` (`api.rs:960`), `type_into` (`:973`), `invoke` (`:951`), waiting on
    `settled` (`:1008`); **no `eval`**. An engine that cannot capture answers `failed` naming it — never a blank picture.
  - Verify: `cargo test -p cobolt-web-preview --no-fail-fast` on a served fixture page — the PNG's size equals the requested viewport × dpr;
    a script that clicks, types, waits and reads a property answers the values, text and errors; skips with a printed reason without a
    display, and **fails** instead of skipping when `PRC_REQUIRE_WEBVIEW=1` (the CI switch, plan §5).
  - **Stopped here (2.0.121), for the operator:** (1) the steps cannot go through `dispatch` / `type_into` on a served page — both refuse any player that is not `cfg.headless` (`cobolt-web-player/src/api.rs:1038-1060`: "dispatch() is for headless runs; a page raises events itself"), and a page that `render` can draw is a mounted, non-headless player. The ways out each decide something outside this spec: relax the player's gate for a driven page (WASM-016's API and its security surface), drive the rendered DOM instead (real `click`/`input` events, contrary to plan §2.4's mechanism), or run the player unmounted (nothing to render). (2) wry 0.57 exposes no snapshot on any engine (no `snapshot`/`takeSnapshot`/`CapturePreview` in its sources), so `render` needs direct platform crates in the helper (`objc2-web-kit` for `takeSnapshotWithConfiguration`, `webview2-com` for `CapturePreview`, `webkit2gtk` for `get_snapshot`) — new dependencies, two of three platforms not runnable here. T42–T48 are not started; T49's version and CHANGELOG were done for T30–T40 (2.0.121), its box stays open.
  - **Operator decision (2026-11-01):** (1) **real DOM events** — the `script` steps `click` and `type` fire real `click` / `input` events on the rendered page through a fixed step runner built into the helper (a closed set of steps, still **no `eval`** of caller text); the player's headless gate on `dispatch` / `type_into` stays as it is, and plan §2.4's mechanism is rewritten to match. (2) **all three snapshot engines now** — the helper takes `objc2-web-kit` (`takeSnapshotWithConfiguration`, macOS), `webview2-com` (`CapturePreview`, Windows) and `webkit2gtk` (`get_snapshot`, Linux) as direct, target-gated dependencies, licences recorded as in T42; macOS is measured here, Windows and Linux are compile-gated and run in CI (T45), stated as such. An engine that fails at run time still answers `failed` naming it, never a blank picture.
  - Done (2.0.122), per the decision; plan §2.4 rewritten to match. `WP/src/main.rs`: module `steps` — `parse_step`/`parse` check a closed set (`click`, `type`, `wait`, `get`, `text`, `style`, `rect`, `focused`, `key`, `snapshot`; control names and roles `form`/`snackbar`/`layer:<NAME>` and property names are checked words, text goes as JSON data; `wait` capped at 30 s), `RUNNER` is the fixed page-side step runner installed with the init script (non-writable, non-configurable `window.__prcSteps`; `click` = pointer/mouse down, focus, up, `click` at the centre; `type` = native value setter, `input`, `change`; settles after each: no DOM change for 50 ms, at most 2 s; `text` = the page's `console.log`/`info` lines during the script, i.e. `DISPLAY`; errors = the page's errors and `console.error` plus each failed step), answering over `window.ipc` under a per-script number the page cannot predict. `render`/`script` wait for the page's load (wry's `with_on_page_load_handler`) and for each other; a script has `time_limit_s` (default 20, 1–120), a render 15 s, then `failed`. Module `snapshot`: macOS `WKSnapshotConfiguration` + `takeSnapshotWithConfiguration`, the image redrawn into an `NSBitmapImageRep` of `width×dpr` by `height×dpr` and written as PNG; Windows `ICoreWebView2Controller3::SetRasterizationScale(dpr)` with monitor-scale detection off, the window `width×dpr` physical pixels, `CapturePreview` (PNG) into `SHCreateStreamOnFileEx`; the answer's pixels are read back from the PNG header. `WP/Cargo.toml`: `objc2`, `block2`, `objc2-foundation`, `objc2-app-kit`, `objc2-web-kit` (macOS) and `webview2-com`, `windows` (Windows) at the versions wry already locks — `Cargo.lock` gains 7 dependency lines and no new package. Linux's `get_snapshot` comes with T42 (the helper has no Linux run loop before it). `snapshot` answers an error naming WASM-015: the host page does not hand the player's state to the page (`cobolt-web-bundle/src/hostpage.rs`, `mount` keeps `app` local). `PT/src/web/preview.rs`: `Command::{Render, Script}`, `Event::{Rendered, Scripted}`, and `PreviewSession::{ready, render, script}` (blocking, for the headless runner; other events kept for the next `poll`).
  - Verified: `cargo test -p cobolt-web-preview --no-fail-fast` — unit 8/8 (new: `the_render_and_script_answers_are_the_ides_contract`, `a_script_is_a_closed_set_of_checked_steps`, `a_png_header_gives_its_pixels`), `tests/headless.rs` 3/3 on the built helper (macOS WKWebView): `a_render_is_the_viewport_at_the_asked_pixel_ratio` — 390×844 at dpr 3 → a **1170×2532** PNG, 66 030 bytes, 730 ms from `open`; `a_script_types_clicks_waits_and_reads_through_real_dom_events` — on a fixture drawn 300 ms after the load into `#pr-app`'s open shadow root (where the built page's player draws; the runner searches it, and `{"wait":{"control":..}}` waits for a control to appear): layer hidden `false`, typed `Ana`, click → label `Hello, Ana`, input value, computed colour, rect, focused `BTN-GO`, `text` = `CLICKED Ana`, a missing control and `snapshot` reported as step errors, an `eval` step refused; about 6.4 s from `open` (WebKit slows timers in a window no one sees, and each step waits for the page to settle). Both skip with a printed reason without a web view and fail under `PRC_REQUIRE_WEBVIEW=1`. A hand render outside the repository (400×300 at dpr 2) was looked at: the page drawn, 800×600, not blank. `cargo test -p cobolt-project-tools --lib web:: --no-fail-fast` 53/53 (the new lines pinned in `the_lines_the_ide_writes_are_the_helpers_contract` / `the_lines_the_helper_writes_are_understood`). Windows: `cargo check -p cobolt-web-preview --target x86_64-pc-windows-gnu` clean — **compile-checked only, not run** (CI, T45).

- [x] **T42 — The helper on Linux** (R9, R16; plan §2.4, D2)
  - Files: `WP/Cargo.toml` (the target gate at `:39` and the comment `:33-38`), `WP/src/main.rs` (the `cfg`s at `:145`, `:153`, `:190`,
    `:195`, `:204` — Linux gets the real `main`/`run`; the fallback stays for other systems), `THIRD_PARTY_NOTICES.md`.
  - Do: wry's WebKitGTK 4.1 backend; licences from `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview`
    recorded (wry/tao/webkit2gtk/gtk bindings MIT/Apache; the system WebKitGTK LGPL, dynamically linked); no `ring` / `aws-lc-rs`.
  - Verify: `cargo test -p cobolt-web-preview --no-fail-fast` green on macOS; the `cargo tree` output and each licence written into this
    task when done; `cargo tree … | grep -E "ring|aws-lc"` empty. The Linux build and tests run in CI (T45) — **not runnable on this machine**,
    stated as such.
  - Done (2.0.122). `WP/Cargo.toml`: the gate is `cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))`; Linux adds `webkit2gtk = "=2.0.2"` (`v2_38`, wry's own pin) and `cairo-rs 0.18` (`png`) for the snapshot. `WP/src/main.rs`: Linux gets the real `main`/`run` (a missing `DISPLAY`/`WAYLAND_DISPLAY` answers `failed` + `closed` and exits 3, instead of GTK failing inside); `build_view` puts the web view into tao's `default_vbox` with `build_gtk` on Linux, the raw handle elsewhere; `snapshot` on Linux is `WebViewExt::snapshot(Visible)` scaled onto a cairo surface of `width×dpr` by `height×dpr` and written as PNG. The fallback `main` stays for other systems. `THIRD_PARTY_NOTICES.md`: entries for wry/tao, the objc2 family, webview2-com/windows, the WebKitGTK/GTK/cairo bindings (system libraries LGPL, dynamically linked, not distributed) and `option-ext` (MPL-2.0, through wry's `dirs`).
  - `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview`: direct `cairo-rs 0.18.5` MIT, `serde_json 1.0.150` MIT OR Apache-2.0, `tao 0.35.3` Apache-2.0, `webkit2gtk 2.0.2` MIT, `wry 0.57.0` Apache-2.0 OR MIT; the bindings `webkit2gtk-sys`, `javascriptcore-rs 1.1.2`, `soup3 0.5.0`, `gtk`/`gtk-sys`/`gdk 0.18.2`, `gio 0.18.4`, `glib 0.18.5` all MIT. Over the 124 crates of the tree: 58 MIT OR Apache-2.0, 34 MIT, 18 Unicode-3.0 (ICU), 6 Apache-2.0 OR MIT, one each of Apache-2.0 (tao), MIT/Apache-2.0, Apache-2.0 AND MIT, MIT OR Apache-2.0 OR Zlib, Unlicense OR MIT, (MIT OR Apache-2.0) AND Unicode-3.0, and one MPL-2.0 (`option-ext 0.2.0`, recorded). `… | grep -E "ring|aws-lc"`: **0 lines**. macOS adds `objc2 0.6.4`/`block2 0.6.2`/`objc2-foundation 0.3.2` MIT, `objc2-app-kit`/`objc2-web-kit 0.3.2` Zlib OR Apache-2.0 OR MIT; Windows `webview2-com 0.39.1` MIT, `windows 0.62.2` MIT OR Apache-2.0. `Cargo.lock` gains 9 dependency lines, no new package.
  - Verified: `cargo test -p cobolt-web-preview --no-fail-fast` on macOS — 8/8 + 3/3. `cargo check --target x86_64-pc-windows-gnu` clean. **Linux not compiled or run on this machine**: `cargo check --target x86_64-unknown-linux-gnu` stops at `atk-sys`'s build script (pkg-config has no Linux GTK sysroot here); the Linux build and tests are the CI's (T45).

- [x] **T43 — `render_form` and `run_form` for a web form** (R9, R16; AC8, AC15, AC9; plan §2.4)
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
  - Done (2.0.122). New `PT/src/tools/web_form.rs`: `render` and `run` for a `.wcfrm`/`.mcfrm` (`is_web_form`), routed from `tools/mod.rs` (`render`, `run_form_in`; `in_shell` refused for a web form). Per call: `helper_path()` first (none → an error naming `prc-web-preview`, never a picture); `web::backend()` must be linked (else `NOT_YET_LINKED_MESSAGE`); `WebBuildRequest::from_manifest` with `out_dir` moved to a folder of its own under the system temp folder (removed afterwards — `dist/web` and the sources are not written); served with the project's isolation headers, no live reload; `PreviewSession::start` on `<url>?pr-form=<FORM-NAME>` at the designed size (or `width`/`height`), `dpr` = `scale`; `ready` — a window reported visible is refused (AC15); a `{"wait":{"control":"form"}}` step first (the player draws after the load; the helper gained `wait` on a control, and the runner searches `#pr-app`'s shadow root, where the player draws); then `render`, or the steps. `run_form` steps are translated (`set` Text/Value → `type`, `event` onClick → `click`, `wait_ms` → `wait`, `read` → `get`; the helper's own steps pass; `dump`, `open_form` and other events are reported not done, with why). Answers: render = the desktop keys (`form`, `width`, `height`, `scale`, `note`, `project`, `window` when asked, `unsaved`) + `engine` (`webkit` / `webview2` / `webkitgtk`); run = `form`, `steps`, `reads`, `program_ended`, `on_pane`, `windows`, `timed_out`, `elapsed_ms`, `picture`, `display` (the page's `console.log`, i.e. `DISPLAY`), `project`, `runtime_error` when the page reported errors, + `engine`. `PreviewSession::ready` now returns the reported visibility. `content.rs` (`web-kinds`, the web skill's build step) rewritten: the tools draw a web form with the system web view, headless; the developer's Preview is the browser or a device.
  - Verified: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` **15/15**. New `render_form_and_run_form_draw_a_web_form_with_the_system_web_view_008` on the Web template built by the real generator (`GeneratorBackend`, the committed player): with `PRC_WEB_PREVIEW` naming nothing, both tools answer an error naming `prc-web-preview` and no image; with the built helper, `render_form`'s key set = the desktop set + `engine` (`webkit`), `dist/web` not created; **AC8: 5 renders at 960×640, worst 0 of 614 400 pixels over 2 (0.000 %), largest channel difference 0**; first render 4–10 s (build included); the picture looked at — the template form in its shell, drawn; `run_form` typed `Ada`, clicked Greet, read `LBL-RESULT::Caption` = `"Hello, Ada"`, `dump` reported not done, key set = desktop + `engine`, about 9.7 s. **AC9:** the desktop cases (`render_form_answers_with_an_image` and the other 13) unchanged and green. **AC15:** the helper's `ready` says `visible:false` (pinned in `cobolt-web-preview` `tests/headless.rs`), and a visible one is refused. `cargo test -p cobolt-project-tools --lib --no-fail-fast` 127/127. `PT/tests/kinds.rs` `every_form_tool_takes_the_web_and_mobile_form_extensions` moved to this contract (it asserted the desktop stubs answered for `.wcfrm`/`.mcfrm`): the desktop renderer and runner now fail if called, and with `PRC_WEB_PREVIEW` naming nothing both tools answer an error naming `prc-web-preview` — `--test kinds` **9/9**. Skips with a printed reason when the helper is not built or there is no web view; fails under `PRC_REQUIRE_WEBVIEW=1`.

- [x] **T44 — Layers start hidden** (R11, AC10; plan §1 R11 row)
  - Files: `PT/tests/tools.rs` (a layered `.wcfrm` fixture, run headless).
  - Do: nothing in the Preview's code — it holds no layer state; the form description carries no layer `Visible`
    (`cobolt_forms::model::Layer`).
  - Verify: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` — each layer's visibility read by `script` is `false` at open and
    after an `app.update` restart, `true` only after the program's `SET … Visible TO TRUE` handler runs.
  - Done (2.0.122). Nothing in the Preview's code (it holds no layer state); `cobolt_forms::model::Layer` has only `name` and `backdrop`, so no description can start a layer shown. New `layers_start_hidden_in_the_web_view_and_after_an_update_008` in `PT/tests/tools.rs`: a layered `.wcfrm` (LYR-SEE, LYR-TOP; handlers `MOVE 1 TO LYR-SEE::Visible` / `LYR-TOP::Visible`) built by `GeneratorBackend`, served with live update, driven in the helper (`ready` → `visible: false`): both layers `false` at open; after the BTN-SEE click `[true, false]`; then the form is changed (a caption), rebuilt into the served folder and `notify_changed()`, and once the changed form is running (BTN-SEE reads `See it`) both are `false` again; both `true` only after their own handlers. Helper change found by it: a script running when the page starts loading again now answers `failed` ("the page loaded again while the script ran") at once, instead of waiting out its time limit (`UserEvent::Loading`, from wry's `PageLoadEvent::Started`).
  - Verified: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` **16/16**; the layers test run 5 times in a row, 5/5 green (new form running 435–1693 ms after the notice). **Finding for T47 / WASM-016 R16 (not fixed here):** the changed form reached the page by the Preview script's full reload in 3 of those 5 runs, and in the other 2 the in-place `app.update` answered `unchanged` although the served `config.json` had a new form hash (checked over raw HTTP while diagnosing), followed by a restart; only 1 of the earlier diagnostic runs saw an in-place `program` update (83.7 ms). The dev server's folder watcher firing during the rebuild races `notify_changed`. AC12's "no page reload" is for a property, a class rule and the stylesheet, so T47 will measure it; layers are hidden after every restart path.

### Shipping and the System KB

- [x] **T45 — The helper ships; its runtime libraries are declared** (R9, R16; spec §6; plan §6)
  - Files: `.github/workflows/build-all-platforms.yml` (no `prc-web-preview` today; `.deb` `Depends:` at `:1016` names only
    `libasound2 | libasound2t64`; `.rpm` `Requires:` at `:1155` only `libasound.so.2()(64bit)`; the requirements line at `:1376`),
    `IDE/src/toolchain.rs` (`detect`, `:204`).
  - Do: build and package `prc-web-preview` beside the IDE on macOS, Windows, Linux; `.deb`/`.rpm` gain WebKitGTK 4.1, the Windows text names
    WebView2 — each stated as needed for **agent and test renders** only (follow the `installer-look` skill for the package text); the CI runs
    `cargo test -p cobolt-web-preview` with `PRC_REQUIRE_WEBVIEW=1` (Xvfb on Linux). `detect` reports a missing WebKitGTK/WebView2 as
    "render_form/run_form cannot draw web forms", never as a Preview or Build blocker; its text is a `Tr` key in six languages.
  - Verify: the packaging steps list `prc-web-preview` next to the IDE on all three systems (read from the workflow file; the run is the CI's);
    `cargo test -p cobolt-ide --bin cobolt-ide toolchain --no-fail-fast` green.
  - Done (2.0.122). `.github/workflows/build-all-platforms.yml`: the Linux build installs `libwebkit2gtk-4.1-dev` and `xvfb` (the comment says why); the build step adds `-p cobolt-web-preview`; a new step **Test the headless web view helper** runs `cargo test --release --locked --target <target> -p cobolt-web-preview --no-fail-fast` with `PRC_REQUIRE_WEBVIEW=1` on all four runners (under `xvfb-run -a` on Linux; all four targets are native to their runners); `prc-web-preview` is staged beside the IDE and `rcrun` on Linux/Windows (so the `.tar.gz`, `.zip`, MSI — `heat` harvests the staged folder — `.deb` and `.rpm` carry it) and into `PowerRustCOBOL.app/Contents/MacOS/` on macOS, ad-hoc signed with the others. `.deb`: `Recommends: libwebkit2gtk-4.1-0`; `.rpm`: `Recommends: libwebkit2gtk-4.1.so.0()(64bit)` with the comment — weak, not `Depends`/`Requires`, because only agent and test renders need it; the release notes' requirements line says WebKitGTK 4.1 is needed only for agent and test renders of web forms, and that Windows uses the WebView2 runtime Windows 10 and 11 include. `IDE/src/toolchain.rs`: `WebView`, `detect_web_view()` (macOS: present; Linux: `libwebkit2gtk-4.1.so.0` in the four usual library folders; Windows: the WebView2 runtime's EdgeUpdate client key, HKLM 64/32-bit and HKCU), `web_view_with` (the decision, testable). `app.rs` startup: a missing web view pushes one Output note, `Tr::status_web_view_missing` in six languages (`{what}` named) — only render_form / run_form are affected; nothing blocks Build or the Preview.
  - Verified: the workflow read back (`prc-web-preview` at the build, test, Linux/Windows staging, macOS bundle and codesign lines; the run itself is the CI's, **not run here**). `cargo test -p cobolt-ide --bin cobolt-ide toolchain --no-fail-fast` **21/21** (new `toolchain_web_view_is_found_in_any_place_or_named_when_missing`; this Mac: `Present`); `… i18n --no-fail-fast` **21/21**; no new warning in the touched files.

- [x] **T46 — The System KB stays true** (steering: the System KB rule)
  - Files: `assets/knowledge/chunked.data`, only if T43's `content.rs` change turns the freshness test red.
  - Do: no control, property, method or event changes, so no compiler doc table changes.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` (`grace_host.rs:4780`); if
    red, `cargo run -p cobolt-ide --example build_chunked_kb` in the same change, then green.

## Acceptance evidence

- [x] **T47 — AC11, AC12, AC5, AC14 driven headless on the real build** (R12, R13, R3, R5, R7, R15; plan §7)
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
  - **Stopped here (2.0.122), for the operator.** T47's checks need what the helper's closed step vocabulary and protocol (T41, shared with WASM-015 plan §3.2) do not have, and widening either is a decision about that no-`eval` surface: (1) **AC5** reads the build-error overlay back by `script`, but the overlay (`.prc-preview-errors`, `devserver.rs` Preview script) is in the page's `document`, outside `#pr-app`'s shadow root where the runner looks, and has no `data-name`; the roles are only `form`, `snackbar`, `layer:<NAME>`. (2) **AC14** reads `innerWidth` and `crossOriginIsolated` inside `/__prc/phone`'s `<iframe>`; the runner is top-frame only, with no frame addressing and no read of a window property. (3) **AC12** asserts "no page reload, the player instance unchanged"; the protocol carries no reload signal to the IDE (the helper sees wry's `PageLoadEvent::Started`, since T44, but only fails a running script on it). T44 also found that a changed form reached a running page by a full reload in 3 of 5 runs, and that `app.update` answered `unchanged` although `config.json` had changed (a race between the dev server's folder watcher and `notify_changed`), which AC12 would measure. Possible ways out, each the operator's call: new roles (`preview-errors`, `phone-frame`) and a read-only `page` step for a fixed list of window properties (`innerWidth`, `crossOriginIsolated`) in the closed vocabulary; a `loading` event in the helper protocol; and whether the update race is fixed here or under WASM-016 R16. T48, T49's sweep and T11 wait with it; T49's version and CHANGELOG were done for T41–T46 (2.0.122), so its box stays open.
  - **Agent decision under the operator's delegation of 2026-11-01** ("do what is better for the project in the long run, not what is easier or safer"). Widen the closed vocabulary and the protocol, keeping it closed — every addition is a fixed name, never caller text, and **no `eval`**:
    (1) **Two new roles**, `preview-errors` (the build-error overlay `.prc-preview-errors`, found in the page's `document`, outside `#pr-app`) and `phone-frame` (the `<iframe>` of `/__prc/phone`). The `text`/`get` steps read them like any role.
    (2) **A read-only `page` step** answering a fixed list of window properties — `innerWidth`, `innerHeight`, `devicePixelRatio`, `crossOriginIsolated`, `location.pathname` — with an optional `frame` naming a role (only `phone-frame` today). Any other name is refused at parse time. A step that reads a window property is what an agent checking a mobile layout needs as much as this test does.
    (3) **A `loading` event in the helper protocol**, raised on wry's `PageLoadEvent::Started` with the URL, sent to the IDE and the tools whether or not a script runs, so "no page reload" is an assertion and not an inference.
    (4) **The update race is fixed here**, not deferred: the dev server's folder watcher and `notify_changed` are two writers of the same truth, and a Preview that reloads 3 times in 5 defeats R13's in-place promise. One build generation number owned by the dev server; `app.update` compares against the generation that finished building, never a file it reads mid-write. AC12 then asserts 5 of 5 in-place.
    WASM-015 plan §3.2 shares this vocabulary: the same roles and the `page` step are the ones its read-only steps use, recorded there when WASM-015 runs.
  - Done (2.0.123). (1) + (2) `crates/cobolt-web-preview/src/main.rs`: the runner's `find` knows `preview-errors` (`.prc-preview-errors` in the page's `document`) and `phone-frame` (`#pr-phone`); a `page` step (`steps::PAGE_PROPERTIES`: `innerWidth`, `innerHeight`, `devicePixelRatio`, `crossOriginIsolated`, `location.pathname`; `frame` only from `steps::FRAMES` = `phone-frame`, waiting up to 10 s for the frame's document), any other name refused at parse time; still no `eval`. (3) `lines::loading` — `{"event":"loading","url":..}` said on every `PageLoadEvent::Started`, script or not; `cobolt_project_tools::web::preview::Event::Loading { url }` reads it (both sides pinned). (4) The race had **two** causes, both fixed here: (a) `devserver.rs` — one writer of the change count: `ServerHandle::build_started` stops the watcher counting while a build writes the folder (the generator's `replace_dir` swaps the whole folder), `notify_changed` counts the build's end once and re-takes the watcher's baseline, `build_failed` counts nothing; `apply_form_stylesheet` brackets its own write; the IDE's `spawn_build` / `finish` bracket every build into a served folder (`IDE/src/panels/web_preview.rs`), and T44's test brackets its rebuild the same way. (b) Found by T47 on the real build: with the race gone, every property change still reloaded the page — `app.update` (`crates/cobolt-web-bundle/src/hostpage.rs`, WASM-016 R16) terminated the running program worker and created the new one in the same task, and WebKit failed to load that worker (a plain `Event` of type `error`, while a `fetch` of the same `worker.<hash>.js` answered 200 with COEP and CORP — measured with a temporary probe, removed). The new program now starts **before** the running one is let go, and a new program that cannot start leaves the running application as it was.
  - Verified: `PRC_REQUIRE_WEBVIEW=1 cargo test -p cobolt-ide --bin cobolt-ide web_preview --no-fail-fast` **19 passed, 0 failed, 1 ignored** (the ignored one is the manual proof run). New `acceptance_on_the_real_build_in_the_headless_helper_008` (about 26 s), on a hello-world Web project and a Mobile one built by the real generator with the committed player (`RealBuild`, a stand-in that wraps `GeneratorBackend` and refuses on request):
    **AC11** — default: "Opened http://127.0.0.1:…/ in the system's default browser." (first build 1.5 s); named: "Opened … in Google Chrome." (the first present known browser, reusing the server); a scripted click on BTN-GREET set LBL-OUT to `Hello`; the interactive Preview starting no helper stays T35's assertion.
    **AC12** — 5 of 5 property changes and 7 of 7 changes in all arrived in place, one page load in the whole run (the helper's `loading` events counted): property `program` 71.8 / 29.0 / 26.8 / 24.9 / 29.5 ms, class rule (`webforms/application.css`) `stylesheet` 6.1 ms, form stylesheet (`apply_form_stylesheet`) `stylesheet of main-form` 8.2 ms — all under D6's working 500 ms (Q5 *to confirm*: reported, not asserted); the new Caption read back after each property change, unchanged after the stylesheet. A thrown error (a served `thrower.js`; the bundle's CSP takes `'self'` only), the player's `console.error` and a COBOL run-time error (BTN-ERR subscripts past `OCCURS 3`: code 1100, unit `BTN-ERR onClick`, line 190) each carried `form` = `main-form` and became Output lines `main-form: …`. Inside the helper the Preview script posts over `window.ipc` (T33); the test posts the same JSON to `/__prc/log`, as a browser sends it, and reads the notes.
    **AC5** — the stand-in refusing the next build (the eighth, after AC12's): `BuildRefused`; the `preview-errors` overlay read back by `text`: "Dismiss | The build failed; this is the last good version. | … webforms/main-form.wcfrm:3: broken on purpose by the test"; LBL-OUT still `Ready 5`, the last good bundle.
    **AC14** — a Mobile project with `NoDeviceTools`: `/__prc/phone` opened, `NotTheDevice` at 390; the frame's `innerWidth` **390** = the Phone preset; the application at `/` in the frame. **Deviation from this task's text, stated:** `crossOriginIsolated` inside the frame is `false` for a Mobile project, because a Mobile build is served with **no** isolation headers by design (`WebBuildRequest::from_manifest` sets `Isolation::Off`, pinned by `a_mobile_request_takes_its_engines_and_variants_from_the_platform`: "a shell serves no isolation headers"). What plan §9 promises — the phone page carries the bundle's headers, so the frame is isolated whenever the bundle is — is asserted both ways: the Mobile frame equals its bundle's setting (`false`), and `/__prc/phone` over the Web project's `require-corp` server gives `crossOriginIsolated` **true** in the frame and around it, `innerWidth` 390.
    Also: `cargo test -p cobolt-web-preview --no-fail-fast` 8 + 3 (headless) passed; `cargo test -p cobolt-project-tools --lib --no-fail-fast web::` 54 passed (new `a_build_is_counted_once_at_its_end_never_in_mid_write_008`: a whole-folder swap during a build counted 0 times, its end once, a failed build 0, a later hand edit once); `cargo test -p cobolt-web-bundle --no-fail-fast` all green (29 result lines). The three Backend tests of the filter need `target/debug/rcrun` (`cargo build -p cobolt-cli --bin rcrun`); with it built they pass (9/9 of `web_preview::tests::a_`).

- [x] **T48 — AC9 and AC15 on the whole IDE** (R10, R16)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`, every `test result` line read — the desktop Preview window
    (`show_preview_window`) and `ExternalFormRun::spawn` tests unchanged and green; no web action for a Desktop project; no interactive
    path spawns `prc-web-preview` (the T35 assertion). Failures that pre-date this spec are named, not hidden.
  - Verified (2.0.123, `PRC_REQUIRE_WEBVIEW=1`, `rcrun` built): **1931 passed, 5 failed, 5 ignored** (one `test result` line). The desktop
    form-run paths green (`form_runtime::built_app_run_tests::*`, `form_codegen_roundtrip_tests::*`, `run_form_dir_tests::*`); a Desktop project
    draws no web settings and keeps its catalogue (`web_settings::tests::a_desktop_project_draws_nothing`,
    `toolbox::project_kind_tests::*`); `preview_opens_the_projects_browser_reuses_its_address_and_starts_no_helper` green; T47's acceptance
    test green inside the sweep. The 5 failures are the line's known pre-existing reds, none in code this spec touches:
    `docs_embed::tests::every_document_ships_in_every_language` (translations deleted by GOLDEN RULE #8),
    `panels::editor::completion_text_tests::every_completion_description_exists_in_six_languages` (English-only WebSocket built-in
    descriptions), `claude_code::bundle::tests::claude_validates_the_written_bundle` (the installed `claude` validator asks for a README),
    `external_crates_service::flow_tests::live_resolver_verdicts` and `…::compatible_and_reserved_collisions_still_refuse` (live crates.io).

- [ ] **T11 — Once, by the operator** (R12, R14, R15; AC11, AC13, AC14 — the look)
  - Preview a Web project in the default and in a named browser; edit the stylesheet and watch it change; break the build and see the
    overlay; close the IDE and see the stopped banner; Preview a Mobile project with an emulator (after WASM-012) and without one. An agent
    never drives the IDE or a browser window (operator ruling, 2026-07-15); it uses the headless helper and the MCP tools.

## Finalize

- [x] **T49 — Version, CHANGELOG, tests of the touched crates** (all)
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
  - Done: 2.0.122 carried T41–T46's version and entry; **2.0.123** (`version.rs`, bumped once) and one `CHANGELOG.md` entry above
    2.0.122 for T47–T49. No documentation task (2.x.x: `README.md` and `docs/` untouched).
  - Verified (`df -h .` first: 27 GiB, then 16 GiB at the lowest, 38 GiB at the end): `cargo test -p cobolt-compiler --no-fail-fast` 12 result lines,
    **232 passed, 0 failed, 5 ignored**; `PRC_REQUIRE_WEBVIEW=1 cargo test -p cobolt-project-tools --no-fail-fast` 20 result lines, **208
    passed, 0 failed, 1 ignored** (lib 128, `tools` 16 with T43/T44); `cargo test -p cobolt-web-preview --no-fail-fast` **8 + 3**;
    `cargo test -p cobolt-web-bundle --no-fail-fast` (touched by T47) all 29 result lines green; `cargo test -p cobolt-ide --bin cobolt-ide
    --no-fail-fast` **1931 passed, 5 failed (pre-existing, named at T48), 5 ignored**, and at 2.0.123 `version`, `i18n`, `web_preview`
    **50 passed, 0 failed, 1 ignored**. No warning in the touched files. `rustfmt --check` (default width): `hostpage.rs` clean; the other
    touched files were not rustfmt-clean before (HEAD: 63 / 39 / 74 / 129 / 64 hunks in `cobolt-web-preview` `main.rs`, `preview.rs`,
    `devserver.rs`, `web_preview.rs`, `tests/tools.rs`); the new code follows their wide style and adds 5 / 0 / 5 / 21 / 0 long-line hunks,
    left as the neighbouring code is rather than re-wrapped alone.

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
