<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The web-view Preview

- **Status:** revised 2026-10-22 to match the spec at 2.0.29 (R1 now names the prebuilt player, R3 names the player's in-place update, R11 adds layers, the operator's answers to Q4 and Q5, and WASM-015's decision that its runners are built on this web view). The first plan (2026-10-20) was implemented and merged into `features-2x` at 2.0.20; §0 says what that left and what this revision adds.
- **Spec:** ./spec.md   **Date:** 2026-10-22   **Line:** 2.x.x (`features-2x`), a **feature**.

## 0. What changed since the 2026-10-20 plan, and why

| # | Change | Why |
|---|---|---|
| C1 | R1 is met by **WASM-002's real build**, not by a stand-in: the Preview asks `web::backend()` for a bundle and gets `WasmBuild` (emit the program module, copy the prebuilt player). This plan **consumes** it and does not implement it. | The spec now says "use the prebuilt player (WASM-016), no Rust compilation". WASM-002's plan (`plan.md:27`, row C6) took ownership of the real `WebBuildBackend`, `crates/cobolt-project-tools/src/web/build.rs`. Today `web::link` is called only by tests (`web/mod.rs:393`, `tests/kinds.rs`), so every real Preview ends at `Note::NotLinked`. |
| C2 | R3 is **in place**, no longer a page reload: a change to the stylesheet, a form description or the program module is pushed into the running player; a full reload is the fallback. | The spec now names the player's in-place update (WASM-016 R16) as the mechanism. The player already has `set_stylesheet` / `set_form_stylesheet` (`cobolt-web-player/src/api.rs:1028`, `:1034`), `add_form` (`:821`) and `attach` (`:832`). WASM-016 T15 is waiting for this plan's page-side half. |
| C3 | R6 is **met**: the Preview starts the project's development backend on request with `cobolt_server::dev::DevServer` and hands the page a development token. | `DevServer` now exists (`cobolt-server/src/dev.rs:64` `start`, `:87` `dev_token`, `:92` `log_lines`, `:96` `stop`); WASM-003's plan lists the IDE half as not done (`plan.md:121`). Q4 is decided: a development token may be used. |
| C4 | R7 is **met**: a failed rebuild keeps the last good bundle **and** shows the errors over the page. | The first plan only wrote them to the Output panel (its §8.3), which the spec does not accept. |
| C5 | R9/AC8 are **met here**, on this plan's helper: a hidden web view that renders a picture and runs a script, behind `render_form` / `run_form`. | WASM-015 Q1 is decided (operator, 2026-10-22): "a harness on the web view of WASM-008". The first plan left R9 to WASM-015. |
| C6 | **Linux is in**: the helper builds on Linux with WebKitGTK; the system browser is only the fallback when the helper is absent. | R1 names WebKitGTK; §6 of the spec makes it a runtime dependency to declare in the packages. The first plan had dropped Linux. |
| C7 | AC4: a page error carries **the form's name**, and a COBOL run-time error its COBOL line. | The first plan gave only the JavaScript file and line (its §8.5). The `ERROR` command already carries `form`, `unit` and `line` (WASM-ABI-v1 §5, op `0x04`). |
| C8 | R11 (layers) added. | New in the spec (2.0.26). |
| C9 | The helper is **shipped** next to the IDE by the installers. | `.github/workflows/build-all-platforms.yml` builds and packages no `prc-web-preview` today; without it every installed IDE falls back to the browser. |

Kept unchanged, because they are still right: the separate process (Q1), wry + tao (Q2), the JSON-lines protocol (extended, §3), the loopback server with its path and Host checks, the presets and the controls window with its fixed size, `Note`s rendered from `Tr`, the navigation policy (R8), and "two actions" (Q6).

## 1. Approach

*Build → serve → show → update*, each step behind a seam that already exists.

```mermaid
flowchart LR
  P[Preview / Run] --> J[build job thread<br/>panels/web_preview.rs]
  J -->|web::backend().build| B[WasmBuild<br/>WASM-002: cobolt-wasm + assemble_bundle<br/>+ shipped assets/web-player]
  B --> D[dist/web, replaced atomically]
  J --> S[dev server, project-tools web/devserver.rs<br/>loopback, COOP/COEP, /__prc/*]
  D --> S
  K[DevServer<br/>cobolt-server dev.rs] -. backend addr + dev token .-> S
  S --> H[prc-web-preview<br/>wry + tao, own process]
  W[SourceWatcher] -->|changed| J
  J -->|change set| S
  S -->|/__prc/changes| U[page script: set_stylesheet / add_form / attach / reload]
  H -->|console, error, cobol-error, blocked, closed| O[Output panel + controls window]
  T[render_form / run_form<br/>for a web form] --> HH[prc-web-preview --headless]
```

| Req. | How it is met | State on 2026-10-22 |
|---|---|---|
| **R1** | `WebPreview::start` (`cobolt-ide/src/panels/web_preview.rs:545`) runs `web::backend().build` on a thread (`app.rs:2701` passes the backend), then serves `dist/web` and opens the helper. With WASM-002's `WasmBuild` linked at the two start points (the IDE's `main`, `rcrun mcp`), the bundle is the Build's own bundle: the program module from `cobolt_wasm::module::compile_project` (`module.rs:161`) and the player copied from `assets/web-player/`. No `cargo`, no `rustc`. | Seam done; the real build is WASM-002 T-work. This plan adds the AC1 test that runs it. |
| **R2, AC6** | The web view lives in `prc-web-preview` (`crates/cobolt-web-preview`). The IDE holds only its pipes; `PreviewSession::poll` / `alive` (`cobolt-project-tools/src/web/preview.rs:311`, `:334`) see a dead child and report `Closed`; Preview can be started again. | Done; AC6 test kept. |
| **R3, AC2** | §2.1: the server keeps a **change set** per rebuild (which files differ by content hash: `.css`, a form description, `program*.wasm`, other). The page script, served at `/__prc/livereload.js`, asks `/__prc/changes?since=N` and applies it to the app object that `PowerRustCOBOL.mount` resolves to (`cobolt-web-bundle/assets/app.js:7`): stylesheets → `set_stylesheet` / `set_form_stylesheet`; a description → `add_form` (replace) and re-render of open instances of that form; a module → `attach` of the new module with the form state carried where the data layout is unchanged; anything else (host page, loader, player, manifest with a different form list) → `location.reload()`. Each update posts its own measured time; the IDE prints `Note::Updated { ms, how }`. | Reload done; in-place is new. Needs WASM-016 T15 (the player's replace semantics). |
| **R4, AC3** | `web/presets.rs` (7 presets × 2 orientations, ratio, insets), colour scheme and locale; `Controls::changes_from` (`web_preview.rs:268`) sends `size`, `scheme` and at most one `emulate`. The page's window is the OS's and changes size only on `size` or the developer's drag; the controls window is one fixed allocation. | Done; `the_controls_window_keeps_its_size`. |
| **R5, AC4** | Developer tools: `Command::Devtools`. Console: the helper's init script wraps `console.*`, `error`, `unhandledrejection` and posts them over `window.ipc`. **New:** every event carries `form` (the form on top: the player's route, `#/form`, WASM-ABI-v1 §12.1), and the page script reports the player's COBOL run-time errors (`Player::errors`, `api.rs:915`, fed by the `ERROR` command) as a `cobol-error` event with form, unit and COBOL line. The IDE shows `form: message (unit:line)`. | Console done; form name and COBOL line new. |
| **R6** | §2.2: a **Start backend** action in the controls window (and offered by `Note::BackendOff`) runs `DevServer::start` with the project's `backend/`; its log goes to the Output panel; the served `config.json` for this run names the dev backend's `ws://127.0.0.1:port` and carries the development token. With no backend running, a call answers the A6 error frame (`transport`) and the Preview says the backend is off — never a silent failure. | New. |
| **R7, AC5** | A failed rebuild leaves `dist/web` untouched (the atomic-replace contract on `WebBuildBackend`); the server records the errors and the page script shows them **over** the page in a dismissable overlay (DOM built with `textContent`, never HTML — D14), and the Output panel has them too. The next good build removes the overlay. | Output panel done; overlay new. |
| **R8, AC7** | `may_navigate` in the helper (exact origin prefix with boundary check, tested against `@`, port-suffix and `file:` tricks), new windows denied, both reported as `blocked`; the controls window offers the address in the system browser through `open_in_system_browser` (`preview.rs:172`), which refuses anything but `http`/`https`. The page is given no native interface beyond `window.ipc.postMessage` of the four event kinds. | Done; kept. AC7's choice: **opens in the system browser on request** (not automatically). |
| **R9, AC8** | §2.3: `prc-web-preview --headless` — a hidden window, the same init script and navigation policy, commands `render` (a PNG of the viewport) and `script` (type, click, wait, read a property, return the displayed text, errors and a picture). `render_form` / `run_form` (`cobolt-project-tools/src/tools/mod.rs:520`, `:537`) route a web form there and answer with the same fields as for a desktop form, plus `engine` (WASM-015 R11). | New. |
| **R10, AC9** | Every branch is behind `is_web_project()`; the designer's Preview window (`show_preview_window`) and Run Form (`ExternalFormRun::spawn`) are untouched for Desktop; `render_form`/`run_form` keep the desktop path for `.cfrm` forms. | Done; AC9 tests kept, plus the headless routing test. |
| **R11, AC10** | The Preview adds **no layer state of its own**. The form description has no `Visible` on a layer by construction (`cobolt_forms::model::Layer`, `model.rs:8316`, deliberately without `Visible`); the designer's layer-tab visibility lives only in the IDE; the player starts every layer hidden per instance (WASM-016 R19, WASM-ABI-v1 A9). An in-place description update creates no new instance, so it neither shows nor hides a layer the program has shown; a module or full reload starts over hidden, as the program would. | AC10 test new (headless, §6). |

## 2. The new parts

### 2.1 In-place update (R3)

- **Server** (`cobolt-project-tools/src/web/devserver.rs`): `fingerprint` (`:223`) already notices a change; it gains a per-file content hash table and, per version, a `ChangeSet { css: Vec<path>, descriptions: Vec<form>, module: bool, reload: bool }`. `/__prc/changes?since=N` answers it as JSON (merging the sets between `N` and now; `reload` if `N` is too old). `/__prc/version` stays for older pages.
- **Page script** (`with_reload_script`, `:492`, and the `/__prc/livereload.js` body): polls `/__prc/changes`, fetches the changed files (content-hashed names, so no stale cache), and calls the app object. A call that throws falls back to `location.reload()` and posts why.
- **What "where it can" means (R3):** a stylesheet and a description keep every instance and every value; a new module keeps the open forms when the manifest's `forms[i].region_size` and the data layout fingerprint are unchanged (the player re-attaches and replays nothing), and otherwise reloads. The rule is the player's (WASM-016 T15); this plan only chooses which call to make and reports `how` (`css` / `description` / `module` / `reload`).
- **Budget (Q5):** see §4 D6.

### 2.2 The development backend (R6)

- `cobolt-ide` gains a path dependency on `cobolt-server` (a workspace crate, no new external dependency; `cobolt-cli` already has it, `Cargo.toml:44`).
- `WebPreview` owns an `Option<cobolt_server::dev::DevServer>`; Start/Stop in the controls window; `log_lines` drained into the Output panel each poll; stopped with the Preview and on *open another project*.
- The served `config.json` (the bundle's, `cobolt-web-bundle/src/lib.rs:671` `"backend": {"origins", "unsafe_plain_ws", …}`) is **rewritten on serve** for the Preview run only, like `with_reload_script` rewrites `index.html`: the dev origin, `unsafe_plain_ws: true` (loopback only, WASM-003 R5) and a `dev_token`. The file on disk is never changed, so a Build's bundle cannot carry a token (WASM-003 Q5 decided: never in a release build).
- **Gap stated:** the player has no way to take a token at start today (no `dev_token` in `crates/cobolt-web-player/src`). The player's session must accept `backend.dev_token` from the config **only when `unsafe_plain_ws` is set and the origin is loopback**, and send it as the A8 `auth` message. That is a player change (WASM-016/WASM-009 file `net.rs`), named in tasks as a dependency, not made here.

### 2.3 Headless render and scripted run (R9)

- `prc-web-preview --headless` reads the same JSON lines; the window is created invisible (`tao` `WindowBuilder::with_visible(false)`) at the preset's size.
- New commands: `{"cmd":"render","path":"…png"}` → `{"event":"rendered","path":…,"width":…,"height":…}`; `{"cmd":"script","steps":[{"type":"click","control":"BTN-OK"}, {"type":"type","control":"TXT-NAME","text":"…"}, {"type":"wait","ms":…}, {"type":"get","control":"…","property":"…"}]}` → `{"event":"scripted","values":[…],"text":…,"errors":[…]}`. Steps go through the player's own `dispatch`, `type_into` and `invoke` (`api.rs:955`, `:968`, `:946`) and wait on `settled` (`:1003`), so a step means what a user's action means.
- The picture comes from the engine's own capture (WKWebView snapshot on macOS, WebView2 `CapturePreview` on Windows, WebKitGTK snapshot on Linux), reached through wry's platform handles. Whether a hidden window paints on each engine is **the** risk of this part (§5); the mitigation is a window placed off every screen.
- `render_form` / `run_form` choose the engine by the form's kind (`.wcfrm`/`.mcfrm` in a Web/Mobile project), build the bundle once per call through `web::backend()`, serve it, start the helper headless, and answer the desktop tools' fields plus `"engine": "webkit" | "webview2" | "webkitgtk"`. With no helper, they answer an error that says so — never a desktop picture of a web form.
- Stability (AC8): the reference form is rendered 5 times; the test compares pixels with a tolerance (per-channel ≤ 2, ≤ 0.5 % of pixels differing) and prints the measured difference.

## 3. The helper protocol (JSON lines, pinned from both sides)

Unchanged lines are kept; **new** ones are marked.

```text
IDE -> helper  {"cmd":"open","url":..,"title":..,"width":390,"height":844,"dpr":3.0,"dark":true,"locale":"pt-BR","safe_area":[47,0,34,0]}
               {"cmd":"size",..} {"cmd":"reload"} {"cmd":"scheme","dark":true} {"cmd":"devtools"} {"cmd":"close"}
               {"cmd":"emulate","dpr":2.0,"locale":"pt-BR","safe_area":[24,0,20,0]}
         new   {"cmd":"render","path":..}   {"cmd":"script","steps":[..]}          # headless only
helper -> IDE  {"event":"ready"} {"event":"blocked","url":..} {"event":"failed","message":..} {"event":"closed"}
               {"event":"console","level":..,"text":..,"form":..}                 # form: new, optional
               {"event":"error","message":..,"source":..,"line":..,"form":..}     # form: new, optional
         new   {"event":"cobol-error","form":..,"unit":..,"line":..,"code":..,"message":..}
         new   {"event":"updated","how":"css|description|module|reload","ms":..}
         new   {"event":"rendered",..} {"event":"scripted",..}
```

An old helper ignores an unknown command and an old IDE ignores an unknown event or field, so the two can be one version apart. Each side's tests pin the exact lines (`cobolt-web-preview` tests and `web::preview` tests).

## 4. Key decisions & alternatives

- **D1 — Separate process (Q1, D7 in WASM-INDEX).** Kept. The spec's recommendation; the INDEX lists D7 as **open** — this plan proceeds on it as the working assumption and the operator can still overrule. Rejected: a child web view inside the egui window (not a native container on every OS; a crash takes the IDE).
- **D2 — wry 0.57 + tao 0.35 (Q2, which the spec leaves to `/plan`).** Kept. The same two crates Tauri 2 uses (WASM-012), so the Preview and the mobile shell share an engine library. Probe of 2026-10-20 (unchanged): wry Apache-2.0 OR MIT, MSRV 1.85; tao Apache-2.0, MSRV 1.74; macOS `objc2-web-kit` (Zlib OR Apache-2.0 OR MIT), Windows `webview2-com` (MIT); no `ring` or `aws-lc-rs` through them. **New for Linux:** wry's `webkit2gtk` / `gtk` bindings (MIT) over the system WebKitGTK 4.1 (LGPL, dynamically linked, a system package); the licences are re-read from `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview` at implementation and recorded in `THIRD_PARTY_NOTICES.md`.
- **D3 — In-place update through the app object, chosen by a server-side change set.** Rejected: always reloading (the first plan; the spec now asks for in place); diffing in the page (the page cannot know a module's layout changed — the server and the manifest can).
- **D4 — A token reaches the page only through the served, rewritten `config.json` of a Preview run.** Rejected: putting the token in the init script or `localStorage` (it would survive into a later page); writing it into `dist/web` (a Build could then ship it).
- **D5 — Headless on the same helper (WASM-015 Q1, decided).** One engine for what the developer sees and what the agent sees. Rejected (by the operator's decision): a headless Chromium over WebDriver.
- **D6 — The update budget (Q5).** The operator's answer is "as short as possible", marked *to confirm*, and AC2 needs a number. **Working assumption, not a decision:** a style change shows within **500 ms** of the save (the number WASM-007's plan already set for Apply → computed style, `WASM-007 plan.md:89`), and a description or module change within **500 ms plus the build's own time**, which the Output panel prints separately. To make "as short as possible" real, the IDE watcher's 400 ms look + one quiet look (the first plan's §6) becomes a 150 ms look with one quiet look, and the page polls `/__prc/changes` at 100 ms while the Preview is open. The measured numbers go in §7 and to the operator.
- **D7 — Preview and Run are two actions (Q6).** Kept as the working assumption (the spec's recommendation, not decided): Preview = web view with the device controls and in-place update; Run = the same served bundle in the system browser with the dev backend; Build = the bundle and its report.
- **D8 — Backend during Preview (Q3).** Working assumption: **the real backend started locally** (the spec's recommendation); no fixture stub is built. Not decided; if the operator wants a stub, it is a separate generator and a separate task.
- **D9 — Notes, not strings; the controls window is the sum of constants.** Kept (the first plan's §4).

## 5. Risks & mitigations

- *WASM-002's `WasmBuild` is not linked yet* → the Preview keeps `Note::NotLinked` and still serves an existing `dist/web`; AC1/AC2 cannot be measured until it is; tasks order this plan's AC tests after WASM-002's link.
- *The player's replace semantics (WASM-016 T15) are not there* → the page script falls back to reload for that kind of change and reports `how: reload`; AC2 stays open for that kind until T15 lands.
- *A hidden window does not paint on some engine* (WebView2 is known to suspend rendering of an invisible window) → place the headless window off every screen instead of hiding it; if an engine still refuses, `render_form` answers an error naming the engine — never a fake picture.
- *Linux CI has no display* → the headless runner needs a display server on Linux (Xvfb or a Wayland headless compositor); tests that need the helper are skipped with a printed reason when none is found, so `cargo test` never requires a browser engine for people who do not touch the web kinds (WASM-015 Q1's constraint).
- *A token in the page* → only in a Preview run, only loopback, only when the backend was started by the IDE; the bundle on disk never has one; a test greps `dist/web` after a Preview run.
- *A console flood* → capped at 40 per poll, the log keeps 300 notes (kept).
- *The helper is not beside the IDE* → installers ship it (C9); otherwise `PRC_WEB_PREVIEW` names it; otherwise the system browser with a note naming what is lost.
- *Windows untested on this machine* → the CI matrix builds and runs the helper's tests on Windows; a window there is the operator's to look at.

## 6. Affected crates / files

- `crates/cobolt-project-tools/src/web/devserver.rs` — per-file hashes, `ChangeSet`, `/__prc/changes`, the error overlay in the page script, `config.json` rewrite for the Preview run.
- `crates/cobolt-project-tools/src/web/preview.rs` — new commands and events (§3), `form` fields.
- `crates/cobolt-project-tools/src/tools/mod.rs` (`render_form`, `run_form`) — the web route (§2.3).
- `crates/cobolt-project-tools/src/content.rs` — the tools' descriptions and the plugin rules say a web form is rendered by the system web view (spec §6, System KB / agent kit).
- `crates/cobolt-web-preview/{Cargo.toml,src/main.rs}` — Linux target, headless mode, `render`, `script`, `cobol-error`, `form` on events, `updated`.
- `crates/cobolt-ide/src/panels/web_preview.rs` — `DevServer` ownership and Start/Stop, `Note::Updated { ms, how }`, `Note::CobolError`, `Note::BackendStarted/Stopped`, the faster watcher look.
- `crates/cobolt-ide/src/app.rs` — stop the dev backend with the Preview and on project switch.
- `crates/cobolt-ide/Cargo.toml` — `cobolt-server` path dependency.
- `crates/cobolt-ide/src/i18n.rs` — new `pk_prev_*` keys in **six languages**: backend start/stop/started/stopped, update-in-place lines per `how`, the COBOL-error line, the headless "no web view on this machine" error, and the overlay's title and dismiss text (the overlay strings are served to the page in the IDE's current language).
- `.github/workflows/build-all-platforms.yml` — build and package `prc-web-preview` beside the IDE on all three systems; `.deb` `Depends` (`:1016`) and `.rpm` requirements (`:1150`) gain WebKitGTK 4.1; the Windows installer text states WebView2 (present on Windows 10/11, the Evergreen runtime otherwise).
- The IDE's toolchain check — reports a missing WebKitGTK/WebView2 as "Preview falls back to the system browser", not as a build blocker.
- `THIRD_PARTY_NOTICES.md` — the Linux crates of D2.
- **Not here:** `WasmBuild` (WASM-002), the player's replace semantics and token intake (WASM-016 T15, WASM-009), the module runner (WASM-015 R1). **Never:** `README.md`, `docs/` (2.x.x rule).

## 7. Test strategy

Every test prints what it measured; none asserts a number it did not produce.

| AC | Test (crate) | Asserts and prints |
|---|---|---|
| AC1 | `cobolt-ide --bin` `web_preview` flow with the real `WasmBuild` and the headless helper on a hello-world Web project | build → serve → page; a scripted click changes the label; prints build, serve and first-paint times |
| AC2 | same, three edits: a property, a class rule, the stylesheet | each arrives as `updated` with `how` ≠ `reload` (once WASM-016 T15 is in) and the player instance id unchanged; prints ms per kind against D6's working budget |
| AC3 | `the_controls_window_keeps_its_size` (kept) + helper protocol test | every preset × orientation sends the expected `size` and `emulate`; `devicePixelRatio` read back by `script` equals the preset's; 120 frames, 6 languages, rectangle unchanged; after a simulated grip drag the stored size is kept |
| AC4 | helper test + controller test | a page `console.error` and an uncaught exception arrive with `form`; a COBOL run-time error arrives as `cobol-error` with unit and line; the Output line contains the form name |
| AC5 | controller test with a backend that fails on the second build | the served bundle is still the first; `/__prc/changes` reports errors; the page script builds the overlay (checked by `script` reading its text) |
| AC6 | `a_session_relays_events_and_notices_a_helper_that_died` (kept) + IDE flow | the child is killed; the IDE reports `Closed`; Preview starts again |
| AC7 | `may_navigate` tests (kept) + a headless page with an off-origin link | navigation refused, `blocked` reported, the web view's URL unchanged |
| AC8 | `cobolt-project-tools/tests/tools.rs` web cases | `render_form` / `run_form` on a web form return the desktop tools' field set plus `engine`; 5 renders of the reference form differ within the tolerance, the difference printed |
| AC9 | the existing desktop Preview / Run Form / `render_form` tests, unchanged | green; no web action appears for a Desktop project |
| AC10 | headless run of a layered web form | the scripted read of each layer's visibility is `false` at open and after an in-place description update; `true` only after the program's `SET … Visible TO TRUE` |
| R6 | controller test with `DevServer` on a fixture `backend/` | Start → log lines in the Output panel, served `config.json` names the dev origin and a token; Stop → a call answers the `transport` error frame and the Preview says the backend is off; `dist/web` contains no token afterwards |

Helper-dependent tests skip, with a printed reason, when the helper or a display is absent. Sweeps use `--no-fail-fast` and read every `test result` line (`cobolt-ide --bin cobolt-ide`).

**Manual (the operator's, never an agent's):** open a Web project, press Preview, flip presets, edit the stylesheet and watch it change, start the backend, break the build and see the overlay. An agent does not drive the IDE or the helper's visible window; it uses the headless mode and the MCP tools.

## 8. Steering compliance

- [ ] **i18n:** every new string a `Tr` key in all six languages (§6), including the overlay text served to the page.
- [ ] **Generated-code contract:** unchanged; the Preview consumes the Build's output and regenerates through the same Build path.
- [ ] **Interpreter/binary parity:** no interpreter change; the dev backend is the same server code as the built backend (`dev.rs` module comment).
- [ ] **System KB:** no control, property, method or event changes, so no compiler doc table changes; the tool descriptions in `content.rs` change — if `prebuilt_chunked_kb_matches_the_published_documentation` turns red, regenerate `assets/knowledge/chunked.data` with `cargo run -p cobolt-ide --example build_chunked_kb` in the same change.
- [ ] **PRIME DIRECTIVE:** the page script and the init script are Rust string constants in `devserver.rs` and `cobolt-web-preview/src/main.rs`, as today; no `.js` file is added to the repository by this plan.
- [ ] **User code is sacred:** the Preview never writes into the project's sources; the `config.json` rewrite happens on serve, the file on disk is untouched.
- [ ] **Self-resizing windows:** the controls window keeps its constant size; the page window changes only on a preset or the developer's drag.
- [ ] **Docs:** `README.md` and `docs/` untouched (2.x.x).
- [ ] **Versioning:** feature on `features-2x`; each change bumps `z` in `version.rs` with a `CHANGELOG.md` entry (bookkeeping by the pipeline).
- [ ] **No "cobolt" in user-facing text**; the helper's binary is `prc-web-preview`.

## 9. Open questions (none decided here)

- **Q1** (separate process) — working assumption D1; INDEX D7 still **open**.
- **Q3** (backend during Preview) — working assumption D8: the real local backend; no stub.
- **Q5** (update budget) — answered "as short as possible", **to confirm**; working number D6 (500 ms for a style change, plus build time for the rest) until the operator gives one.
- **Q6** (Preview vs Run) — working assumption D7: two actions.
- **Decided and applied:** Q2 at this `/plan` (wry + tao, D2); Q4 by the operator (a development token, D4 and §2.2).
- **Dependency, not a question:** the player must accept a development token from a loopback Preview config (§2.2) — owned by the player's plan.
