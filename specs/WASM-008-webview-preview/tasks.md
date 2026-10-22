<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The web-view Preview

- **Status:** in progress. T1–T10 were done against the plan of 2026-10-20 and stay checked with their evidence (results at the bottom).
  T14–T29 are this revision's work, against the plan of 2026-10-22 (C1–C9: the real build consumed, in-place update, the dev backend,
  the error overlay, headless render and scripted run, Linux, form name and COBOL line on errors, layers, the helper shipped).
- **Plan:** ./plan.md   **Date:** 2026-10-22

`[x]` was done and verified; `[ ]` is still to do. The first plan's commits (worktree branch `worktree-agent-a97ba65c65571c2df`):
`2eded9b0` (seam, server, presets, protocol), `d63c6faf` (helper process, settings section), `93b9124e` (controller, controls window,
wiring), `d97f4db6` (plugin rules and skills, System KB, regenerated store); merged into `features-2x` at 2.0.20.

`PT` below is `crates/cobolt-project-tools`, `WP` is `crates/cobolt-web-preview`, `IDE` is `crates/cobolt-ide`.

**Working assumptions carried by these tasks (open questions, not decided here):** Q1 → a separate process (plan D1; WASM-INDEX D7 still
open); Q3 → the real backend started locally, no fixture stub (D8); Q5 → *answered "as short as possible", to confirm*: 500 ms for a style
change, 500 ms plus the build's own time for a description or module change (D6), every test prints the measured number rather than
asserting one the operator has not given; Q6 → Preview and Run are two actions (D7). **Decided and applied:** Q2 (wry + tao, D2), Q4
(a development token may be used — operator, 2026-10-22). AC7's choice: an off-origin link is blocked and **offered** in the system browser,
not opened automatically (plan R8 row).

**Depends on other specs' tasks (named, not done here):** WASM-002 T13 (`WasmBuild`, `PT/src/web/build.rs`, which does not exist yet) and
T16 (`web::link` at the IDE's `main` and `rcrun mcp`); WASM-016 T15 (the player's replace semantics for `add_form` / `attach` —
`add_form` takes XML today, `crates/cobolt-web-player/src/api.rs:821`, JSON after WASM-016 T19–T20) and T21 (layers in the player);
the player's intake of a development token (no `dev_token` anywhere in `crates/cobolt-web-player/src` — WASM-016/WASM-009, `net.rs`).

## Done (plan of 2026-10-20)

- [x] **T1 — The build/serve seam** (R1, R10)
  - Files: `PT/src/web/mod.rs`
  - Done: `WebBuildBackend` (build, bundle_dir, serve), `WebBuildRequest::from_manifest`, `BuildReport {bundle_dir, size, startup_ms, warnings, duration_ms}`, `WebBuildError {NotYetLinked, Refused, Failed}`, `link` / `unlink` / `backend` (`mod.rs:279`, `:289`), `NotYetLinked`.
  - Verify: `cargo test -p cobolt-project-tools --lib web::tests`.

- [x] **T2 — The development server** (R8)
  - Files: `PT/src/web/devserver.rs`
  - Done: loopback only, Host-header check, MIME table (`.wasm` = `application/wasm`), COOP/COEP from the project's isolation setting, `/__prc/version` + `/__prc/livereload.js`, 15 spellings of a path outside the bundle refused, symlinks that leave the bundle not followed, GET/HEAD only.
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver` (prints the request timings).

- [x] **T3 — Presets** (R4)
  - Files: `PT/src/web/presets.rs` — 7 presets × 2 orientations, pixel ratios 1/1.5/2/3, safe-area insets that turn with the screen.

- [x] **T4 — The helper process and its protocol** (R2, R5, R8)
  - Files: `WP/`, `PT/src/web/preview.rs`
  - Done: JSON lines pinned from both sides; `open` carries the emulation; `emulate` is one reload; console / error capture; navigation and new-window policy (`may_navigate`, `WP/src/main.rs:95`); the system-browser fall-back (`open_in_system_browser`, `preview.rs:172`); `PRC_WEB_PREVIEW` and `PRC_PREVIEW_NO_BROWSER` (tests).
  - Verify: `cargo test -p cobolt-web-preview` (5) and `cargo test -p cobolt-project-tools --lib web::preview` (5).

- [x] **T5 — The controller** (R1, R3, R4, R7)
  - Files: `IDE/src/panels/web_preview.rs`
  - Done: build job, serve, session, `SourceWatcher`, rebuild + `notify_changed`, `Controls::changes_from`, `Note`s rendered from `Tr`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` (13).

- [x] **T6 — The controls window** (R4, AC3)
  - Done: 420 x 560 in six languages, one size; buttons and checkboxes in fixed boxes; the log takes what is left.
  - Verify: `the_controls_window_keeps_its_size`.

- [x] **T7 — Wiring** (R10, AC9)
  - Files: `IDE/src/app.rs` — designer Preview (toggle), Run (browser), Build (bundle), Stop and *open another project* end it; File ▸ Build label by kind.

- [x] **T8 — Strings** — `pk_build_*` (9), `pk_prev_*` (25), `pk_ps_*` (7) in six languages.

- [x] **T9 — Agent surface** — `build` for a Web project returns the `BuildReport`; `list_files` shows `web_assets`; the plugin rules and skills mention the Preview (see WASM-001 T11).

- [x] **T10 — Final sweep** — results below.

## Replaced by this revision

- **T12 — The real generator** → replaced by **T27** (the generator is WASM-002 T13/T16's; this list only measures AC1–AC2 on it).
- **T13 — R6, R9** → replaced by **T20** (R6, on `cobolt_server::dev::DevServer`, which now exists) and **T21–T22** (R9 on this helper,
  WASM-015 Q1 decided: "a harness on the web view of WASM-008").
- **T11 — A window, once, by the operator** stays open and moves to the end (it is still the only check of how it *looks*).

## To do (plan of 2026-10-22), in execution order

Each task leaves the tree green: new protocol lines are ignored by the other side until used (plan §3), and every helper-dependent test skips
with a printed reason when the helper or a display is absent (plan §7).

- [ ] **T14 — The protocol, extended on both sides** (R3, R5, R9; plan §3)
  - Files: `PT/src/web/preview.rs` (`Command`, `:52`; `Event`, `:99`), `WP/src/main.rs` (its `lines` module and the reader).
  - Do: commands `render {path}` and `script {steps}`; events `cobol-error {form, unit, line, code, message}`, `updated {how, ms}`,
    `rendered {path, width, height}`, `scripted {values, text, errors}`; an optional `form` on `console` and `error`. Parsing is tolerant:
    an unknown command is ignored by the helper, an unknown event or field by the IDE.
  - Verify: `cargo test -p cobolt-web-preview` and `cargo test -p cobolt-project-tools --lib web::preview` — each new line pinned
    byte-for-byte from both sides; an old-shape `console` line (no `form`) still parses; an unknown command and an unknown event are
    skipped, not fatal.

- [ ] **T15 — The helper on Linux** (R1, R2; plan C6, D2)
  - Files: `WP/Cargo.toml` (the target table at line 39 is `cfg(any(target_os = "macos", target_os = "windows"))` today — add Linux with
    wry's WebKitGTK 4.1 backend; the comment at lines 35–38 says Linux is not used "yet"), `WP/src/main.rs` (the `cfg` on `main`, `:190`/`:195`,
    and on `run`, `:204`: the Linux `main` that answers `failed` + `closed` becomes the real one; the fallback stays for other systems),
    `THIRD_PARTY_NOTICES.md` (the Linux crates).
  - Do: record the licences from `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview`; no `ring` / `aws-lc-rs`.
  - Verify: `cargo test -p cobolt-web-preview` green on macOS; `cargo tree … --target x86_64-unknown-linux-gnu` output pasted into the task
    with each licence; the Linux build itself runs in the CI matrix (T25) — **not runnable on this machine**, stated as such.

- [ ] **T16 — The change set and `/__prc/changes`** (R3; plan §2.1, D3)
  - Files: `PT/src/web/devserver.rs` (`fingerprint`, `:223`; the routes beside `VERSION_PATH`, `:44`).
  - Do: a per-file content hash table; per version a `ChangeSet { css, descriptions, module, reload }` (the host page, loader, player or a
    manifest with a different form list → `reload`); `/__prc/changes?since=N` merges the sets since `N`, `reload` when `N` is older than the
    kept history; `/__prc/version` kept for older pages.
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver` — a changed `.css` answers only `css`; a changed description only
    `descriptions` with the form's name; a changed `program*.wasm` `module`; a changed `index.html` `reload`; two changes merged; a stale
    `since` answers `reload`; the time from the file write to the answer printed.

- [ ] **T17 — In-place update in the page, and the faster look** (R3, AC2; plan §2.1, D6)
  - Files: `PT/src/web/devserver.rs` (the page script served at `RELOAD_SCRIPT_PATH`, `:46`; `with_reload_script`, `:492`), `WP/src/main.rs`
    (relays the page's `updated` post), `IDE/src/panels/web_preview.rs` (`WATCH_EVERY` `:49` from 400 ms to 150 ms with one quiet look;
    `Note::Updated { ms, how }`).
  - Do: the page polls `/__prc/changes` every 100 ms while open and calls the app object `PowerRustCOBOL.mount` resolves to: stylesheets →
    `set_stylesheet` / `set_form_stylesheet` (`crates/cobolt-web-player/src/api.rs:1028`, `:1034`); a description → `add_form` (`:821`);
    a module → `attach` (`:832`); else, or when a call throws, `location.reload()` with the reason. Each update posts `updated {how, ms}`.
    Until WASM-016 T15 lands, description and module changes report `how: reload` — honest, not hidden.
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver` (the script names each entry point and falls back to reload);
    `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — `a_change_is_reported_once_the_folders_are_still` updated to the 150 ms look;
    an `updated` event becomes one Output line with `how` and `ms`. The in-place measurement itself is T27.

- [ ] **T18 — A failed build shows its errors over the last good page** (R7, AC5; plan C4)
  - Files: `PT/src/web/devserver.rs` (errors kept per version, answered by `/__prc/changes`; the overlay in the page script),
    `IDE/src/panels/web_preview.rs` (passes the build's errors and the overlay's title and dismiss text in the IDE's current language).
  - Do: a dismissable overlay built with `textContent` only (never HTML, D14); the next good build removes it; `dist/web` untouched by the
    failure (the atomic-replace contract of `WebBuildBackend`).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — a stand-in backend that fails on the second build: the served bundle
    is byte-identical to the first, `/__prc/changes` carries the errors, the Output panel has them; `cargo test -p cobolt-project-tools --lib
    web::devserver` — the page script contains no `innerHTML`/`insertAdjacentHTML`. The overlay read back by a headless `script` is in T28.

- [ ] **T19 — The form's name and the COBOL line on every page error** (R5, AC4; plan C7)
  - Files: `WP/src/main.rs` (the init script tags `console` / `error` / `unhandledrejection` with `form`, the player's route `#/form`,
    WASM-ABI-v1 §12.1), `PT/src/web/devserver.rs` (the page script reads `Player::errors`, `crates/cobolt-web-player/src/api.rs:915`, and posts
    each new one as `cobol-error`), `IDE/src/panels/web_preview.rs` (`Note::CobolError`; the Output line is `form: message (unit:line)`).
  - Verify: `cargo test -p cobolt-web-preview` (the init script carries `form` on all three kinds); `cargo test -p cobolt-ide --bin cobolt-ide
    web_preview` — a relayed `error` with `form` and a `cobol-error` each become an Output line containing the form name, and the COBOL unit
    and line for the second; a flood stays capped at 40 per poll.

- [ ] **T20 — The development backend on request** (R6; plan C3, §2.2, D4)
  - Files: `IDE/Cargo.toml` (`cobolt-server = { path = "../cobolt-server" }` — no new external dependency; only `cobolt-project-tools` is a
    path dependency there today, line 51), `IDE/src/panels/web_preview.rs` (owns an `Option<cobolt_server::dev::DevServer>`; Start / Stop in the
    controls window, offered by `Note::BackendOff`, `:86`; `log_lines` drained each poll; `Note::BackendStarted` / `BackendStopped`),
    `IDE/src/app.rs` (stop it with the Preview and on *open another project*), `PT/src/web/devserver.rs` (`config.json` rewritten **on serve**
    for a Preview run only: the dev origin `ws://127.0.0.1:port`, `unsafe_plain_ws: true`, `dev_token` from `DevServer::dev_token`,
    `crates/cobolt-server/src/dev.rs:87`).
  - Do: `DevServer::start` (`dev.rs:64`) on the project's `backend/`; with none running, the page's call answers the `transport` error frame
    and the Preview says the backend is off. **Dependency, not done here:** the player accepting `backend.dev_token` only on a loopback config
    with `unsafe_plain_ws` (WASM-016/WASM-009, `net.rs`); until it does, the token is served and unused.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` with a fixture `backend/` — Start → log lines reach the Output panel; the
    served `config.json` names the dev origin and carries a token; Stop → `Note::BackendOff`; after the run, a search of `dist/web` finds no
    token (the file on disk was never changed — user code is sacred). `cobolt-server/src/dev.rs` has no tests of its own today, so the IDE
    test is the evidence; `cargo test -p cobolt-server` stays green.

- [ ] **T21 — The helper, headless: render and script** (R9; plan C5, §2.3, D5)
  - Files: `WP/src/main.rs` (`--headless`: the same init script and navigation policy, the window placed off every screen at the preset's
    size — the plan's mitigation for an engine that will not paint a hidden window; `render` through each engine's capture reached by wry's
    platform handles; `script` steps through the player's `dispatch`, `type_into`, `invoke` and `settled`, `api.rs:955`, `:968`, `:946`,
    `:1003`), `PT/src/web/preview.rs` (a headless session type over the same pipes).
  - Verify: `cargo test -p cobolt-web-preview` — a served fixture page: `render` writes a PNG whose size matches the preset; a `script` that
    clicks, types, waits and reads a property answers `scripted` with the values, the displayed text and the errors; an engine that cannot
    capture answers `failed` naming it — never a blank picture. Skips with a printed reason when no display/engine is present (Linux CI
    without Xvfb).

- [ ] **T22 — `render_form` and `run_form` for a web form** (R9, AC8; plan §2.3)
  - Files: `PT/src/tools/mod.rs` (`render_form` `:520`, `run_form` `:537` / `run_form_in` `:553`: a `.wcfrm` / `.mcfrm` in a Web or Mobile
    project builds once through `web::backend()`, serves, runs the T21 helper headless), `PT/src/content.rs` (the tool descriptions and the
    plugin rules say a web form is drawn by the system web view, spec §6), `PT/tests/tools.rs` (web cases).
  - Do: answer the desktop tools' fields plus `"engine": "webkit" | "webview2" | "webkitgtk"` (WASM-015 R11); with no helper, an error that
    says so — never a desktop picture of a web form. Desktop `.cfrm` forms keep their path untouched.
  - Verify: `cargo test -p cobolt-project-tools --test tools --no-fail-fast` — the web answers' key set equals the desktop one plus `engine`;
    5 renders of the reference form differ within per-channel ≤ 2 and ≤ 0.5 % of pixels, the measured difference printed (**AC8**); the
    desktop `render_form` / `run_form` cases unchanged (**AC9**).

- [ ] **T23 — Layers start hidden in the Preview** (R11, AC10; plan R11 row)
  - Files: a headless test beside T21/T22 (`PT/tests/tools.rs`) on a layered `.wcfrm` fixture.
  - Do: nothing in the Preview's code — it holds no layer state; the form description has no layer `Visible` by construction
    (`cobolt_forms::model::Layer`, `crates/cobolt-forms/src/model.rs:8316`). **Depends on** WASM-016 T21 (layers in the player).
  - Verify: the scripted read of each layer's visibility is `false` at open and after an in-place description update, and `true` only after
    the program's `SET … Visible TO TRUE` handler runs.

- [ ] **T24 — Strings in six languages** (spec §6 i18n; plan §6)
  - Files: `IDE/src/i18n.rs` (175 `pk_prev_` occurrences today), `IDE/src/panels/web_preview.rs`.
  - Do: new `pk_prev_*` keys, all six languages, for: backend start / stop / started / stopped; one update line per `how`; the COBOL-error
    line; the headless "no web view on this machine" error; the overlay's title and dismiss text. Lands **with or before** the task that first
    uses each key (T17–T22), so no task leaves a literal in a widget.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — `every_note_reads_in_all_six_languages_without_a_gap` covers every new
    `Note`; `this_file_has_no_literal_text_for_a_widget` green; `the_controls_window_keeps_its_size` green in all six languages with the
    Start/Stop row added (**AC3**, the controls window half).

- [ ] **T25 — The helper ships beside the IDE; its runtime dependencies are declared** (R1, R2; spec §6; plan C9)
  - Files: `.github/workflows/build-all-platforms.yml` (builds and packages no `prc-web-preview` today; the `.deb` `Depends:` at line 1016
    names only `libasound2 | libasound2t64`; the `.rpm` `Requires:` at line 1150 only `libasound.so.2()(64bit)` — both gain WebKitGTK 4.1;
    the Windows installer text names WebView2), `IDE/src/toolchain.rs` (its `detect`, `:204`, checks the Rust toolchain; a missing
    WebKitGTK/WebView2 is reported as "Preview falls back to the system browser", never as a build blocker).
  - Do: follow the `installer-look` skill for the `.deb`/`.rpm` text. The CI matrix runs `cargo test -p cobolt-web-preview` on all three systems.
  - Verify: the workflow's packaging step lists `prc-web-preview` next to the IDE binary on macOS, Windows and Linux (read from the
    workflow file; the run is the CI's); `cargo test -p cobolt-ide --bin cobolt-ide toolchain` green.

- [ ] **T26 — System KB stays true** (steering: the System KB rule)
  - Files: `assets/knowledge/chunked.data` (only if T22's `content.rs` change turns the freshness test red).
  - Do: no control, property, method or event changes, so no compiler doc table changes.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` (`grace_host.rs:4780`); if red,
    `cargo run -p cobolt-ide --example build_chunked_kb` in the same change, then green.

## Acceptance evidence

- [ ] **T27 — AC1 and AC2 on the real build** (R1, R3; AC1, AC2; plan C1, C2)
  - Depends on: WASM-002 T13 + T16 (`WasmBuild` linked); WASM-016 T15 for `how` ≠ `reload` on a description and a module.
  - Do: the `web_preview` flow with the real `WasmBuild` and the headless helper on a hello-world Web project; then three edits: a property,
    a class rule, the stylesheet. Write the measured build, serve, first-paint and per-kind update times into plan.md §7 and report them to the
    operator against D6's working 500 ms (Q5 stays *to confirm*).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — a scripted click changes the label (**AC1**); each edit arrives as
    `updated` with `how` ≠ `reload` and the player instance id unchanged, its `ms` printed and shown in the Output panel (**AC2**).

- [ ] **T28 — AC3, AC5, AC6, AC7, AC9 re-run on the extended helper** (R2, R4, R7, R8, R10)
  - Verify: `cargo test -p cobolt-web-preview` and `cargo test -p cobolt-ide --bin cobolt-ide web_preview` —
    **AC3**: every preset × orientation sends the expected `size` and `emulate`; `devicePixelRatio` read back by a headless `script` equals the
    preset's; the controls window keeps its rectangle over 120 frames; after a simulated grip drag the stored size is kept.
    **AC5**: the T18 overlay's text read back by `script` over the last good page.
    **AC6**: `a_session_relays_events_and_notices_a_helper_that_died` (`preview.rs:468`) plus the IDE flow — the child killed, `Closed`
    reported, Preview started again.
    **AC7**: `may_navigate` tests plus a headless page with an off-origin link — refused, `blocked` reported, the web view's URL unchanged.
    **AC9**: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — the desktop Preview / Run Form tests unchanged and green; no web
    action for a Desktop project.

- [ ] **T11 — A window, once, by the operator** (R1, R4; AC1, AC3 — the look)
  - Open a Web project, press Preview with `prc-web-preview` beside the IDE, flip presets, edit the stylesheet and watch it change, start the
    backend, break the build and see the overlay. An agent does not drive the IDE or the helper's visible window (operator ruling,
    2026-07-15); it uses the headless mode and the MCP tools.

## Finalize

- [ ] **T29 — Version, CHANGELOG, tests of the touched crates** (all)
  - Files: `IDE/src/version.rs` (`VERSION`, line 22; `2.0.29` when this list was written — bump `z` **once** from whatever it is then),
    `CHANGELOG.md`.
  - Do: **one** entry above the previous `## [PowerRustCOBOL 2.0.x]` heading (`CHANGELOG.md:20` today), dated, marked *Written on
    `features-2x`*, saying what changed (in-place update, error overlay, dev backend, headless render and scripted run, Linux helper, the
    helper shipped). **No documentation task:** `README.md` and `docs/` are never touched on 2.x.x; what 2.x.x says about itself stays in
    `specs/`. Update this file's boxes and the spec's AC boxes with their evidence.
  - Verify: `df -h .` first (a worktree sweep needs disk); then, with `--no-fail-fast`, reading every `test result` line:
    `cargo test -p cobolt-project-tools --no-fail-fast`, `cargo test -p cobolt-web-preview --no-fail-fast`,
    `cargo test -p cobolt-server --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo fmt --check` on the
    touched crates; no new warnings.

## Coverage of the acceptance criteria

| AC | Tasks whose verification covers it |
|---|---|
| AC1 — hello-world in a web view, a click changes it | T27 (T5, T21 feed it); T11 the look |
| AC2 — property, class rule, stylesheet update in place, times printed | T16, T17, T27 |
| AC3 — presets, orientation, DPR; the window keeps its size | T6 (done), T24, T28; T11 the look |
| AC4 — `console.error` and an uncaught exception with the form name | T14, T19 |
| AC5 — a broken build keeps the last good Preview with the errors over it | T18, T28 |
| AC6 — killing the helper leaves the IDE responsive, Preview reopens | T4 (done), T28 |
| AC7 — an off-origin link does not navigate the web view | T4 (done), T28 |
| AC8 — headless render and scripted run, same fields, stable image | T21, T22 |
| AC9 — desktop Preview and Run Form as before | T7 (done), T22, T28 |
| AC10 — a layered form shows no layer until the program shows one | T23 |
| R6 (no AC) — the dev backend on request, never a silent failure | T20 |

## Results (T10, 2026-10-20)

- `cargo test -p cobolt-project-tools --lib web::` — 29 of the lib's 100 tests, all passing; `web::preview` 5.
- `cargo test -p cobolt-web-preview` — 5 passed (the helper's `run` module is compiled on macOS, so the wry/tao calls are type-checked).
- `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — 13 passed. What they print: 120 note renderings (20 notes x 6 languages, none empty, none with a `{}` left); build -> serve -> web view in 337-499 ms with a stand-in generator and helper, a form edit rebuilt and republished (`/__prc/version` rose), the controls reached the helper as 5 lines of which 1 was `emulate`; the controls window held 420 x 560 points over 120 frames (6 languages x busy and empty), the log never under 136 points.
- Dev server (debug build, loopback): 200 requests for a 17-byte file in 114 ms (1744 req/s); a 3 MiB `.wasm` 10 times in 157 ms (191 MiB/s); a changed stylesheet moved the change count in 166 ms (watch interval 150 ms); 15 spellings of a path outside the bundle refused, none leaked.
- Whole IDE binary: 1866 passed, 3 failed — the same three that WASM-001 tasks.md names (not related).
- **Not run, and not claimed:** a real window (no agent drives one), the helper on Windows or Linux, AC1 and AC2 with a real generator, AC8.
