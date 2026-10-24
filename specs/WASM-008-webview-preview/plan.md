<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Preview and Run in the browser and on the device; the web view stays headless

- **Status:** revised **2026-10-23** to match the spec as re-scoped by the operator that day (spec §0, R12–R16, AC11–AC15, Q7) and the code at **2.0.49**. Earlier versions: the first plan (2026-10-20), implemented and merged at 2.0.20; the revision of 2026-10-22 (2.0.29), never implemented. §0 says what this revision keeps, drops and adds, and why.
- **Spec:** ./spec.md   **Date:** 2026-10-23   **Line:** 2.x.x (`features-2x`), a **feature**.
- **`tasks.md` is stale:** its open tasks T14–T29 were written for the 2026-10-22 plan. `/tasks` must rewrite them from this plan; this run changes no other file.

## 0. What changed, and why

### 0.1 The operator's ruling of 2026-10-23 (spec §0)

| # | Change | Why |
|---|---|---|
| C10 | **A Web project's Preview and Run open the page in a browser**: the browser the project names (`preview_browser`, D11), else the system's default. The IDE opens no window for the page; the helper's visible window and the IDE's **controls window** (presets, orientation, pixel ratio, insets, scheme, locale, Devtools) are removed. | R12, R13, R16. The browser's own responsive and developer tools replace them (spec §0, fourth bullet). |
| C11 | **A Mobile project's Preview and Run go to an emulator, a simulator or a device** through WASM-012 R3 / R3a. This plan owns the *decision* and the dev server the device loads; WASM-012 owns listing, starting, installing, launching and the device log. | R14; WASM-012 R3a; WASM-012 plan D-Dev (the shell in development mode loads WASM-008's dev server). |
| C12 | **No emulator or simulator → the browser at a phone width**, with a notice in the Output panel, the IDE's status and the page itself that this is not the device and what to install. The width is honest because the app runs inside a same-origin frame of the preset's CSS size, so its media queries see a phone (D12). | R15, AC14. |
| C13 | **The page tells the IDE what happened over HTTP**: console lines, uncaught errors, COBOL run-time errors and in-place update times are POSTed by the served page script to one new dev-server route, `/__prc/log` (D13), and reach the Output panel. Before, they came over the helper's `window.ipc`, which a browser does not have. | R13 (R5's console relay), AC12 (AC4). |
| C14 | **The helper (`prc-web-preview`) becomes headless only**: it opens no visible window in any mode; it is the engine of `render_form`, `run_form` and the tests (R9). The 2026-10-22 plan's `--headless` mode becomes the only mode. | R16, AC15; WASM-015 Q1 (decided: its runner is built on this helper). |
| C15 | **Q7 decided here** (the spec leaves it to `/plan`): the carrier is `preview_browser` in `[web]` **and** in `[mobile]` (a Mobile project has no `[web]` table, `project_kind.rs:446`), and the Mobile fallback uses it. A value is a name from a closed list, or an application path the developer confirms once on this machine (D11). | Spec Q7; the confirmation closes a hole a free path would open (a downloaded project naming an executable that Preview would start). |

### 0.2 What the code did since the 2026-10-22 plan (checked 2026-10-23, 2.0.49)

| # | Change to the plan | Evidence |
|---|---|---|
| C16 | **R1's real build is linked**: the Web Build runs WASM-010's generator (2.0.37), and the Preview hands the build the player shipped beside the IDE. The 2026-10-22 plan's first risk ("`WasmBuild` not linked") is gone. | `web_preview.rs:628-640` sets `request.player_dir = Some(crate::app::web_player_dir())` before `backend.build`; `app.rs` `web_player_dir`. |
| C17 | **R3's in-place update exists** and is the host page's, not a server-side change set: the served script fetches the new `config.json` and calls `app.update(next)`, which replaces the stylesheets, and for a new program module or form description **restarts the application in the same player** (`player.reset`, no page reload), answering `'reload'` only for another player, worker or application style. The 2026-10-22 plan's `ChangeSet` / `/__prc/changes` (its §2.1, D3) is **dropped**. | `cobolt-web-bundle/src/hostpage.rs:872` `async update(next)`; `devserver.rs:484-499` `reload_script`; WASM-016 T15 (2.0.42, "Chrome 26/26"). |
| C18 | **R6's backend is the IDE's existing Run → Start Backend**: `BackendRun` runs `rcrun backend run <project>` as a child (operator's decision of 2026-10-23, WASM-003 T10). The 2026-10-22 plan's in-process `DevServer` and the `cobolt-server` dependency of `cobolt-ide` are **dropped**: the IDE links no server or token code. | `crates/cobolt-ide/src/backend_run.rs` module comment; `app.rs:2772` `start_backend`; `cobolt-cli/src/main.rs:581-590` prints the address, then the development token. |
| C19 | The 2026-10-22 plan cited `cobolt-web-bundle/assets/app.js`; that file is gone (the host page is Rust constants, `hostpage.rs` `loader_js`, `app_js`). Citations are corrected throughout. | `ls crates/cobolt-web-bundle` has no `assets/`. |

### 0.3 Kept from the earlier plans, because still right

The loopback dev server with its path and Host checks (`devserver.rs:440` `host_is_ours`); `open_in_system_browser`, which opens only a loopback `http` address (`preview.rs:172`, `:200`); the `Note`s rendered from `Tr`; R7's overlay over the last good page (2026-10-22 C4); R9 on this helper (C5) with its render/script commands and the hidden-window risk; Linux for the helper (C6); the form name and COBOL line on errors (C7); R11 (C8); the helper shipped by the installers (C9); D4 (a token reaches the page only through the served, rewritten `config.json`); D6 (the working budget); D8 (the real backend, no stub).

**Dropped:** R4's presets and the controls window (C10); AC3, AC6 and AC7 as IDE behaviour (spec §5: no longer apply; AC7 is the browser's); the helper's `size`, `devtools` and visible `open` (C14); the `ChangeSet` (C17); the in-process `DevServer` (C18).

## 1. Approach

*Build → serve → open → update → relay.* Every step but *open* already exists.

```mermaid
flowchart LR
  P[Preview / Run<br/>app.rs start_web] --> J[build thread<br/>panels/web_preview.rs]
  J -->|web::backend().build| B[dist/web<br/>generator + shipped player]
  J --> S[dev server<br/>project-tools web/devserver.rs<br/>loopback, COOP/COEP]
  B --> S
  K[BackendRun<br/>rcrun backend run] -. address + dev token .-> S
  S --> L{kind}
  L -->|Web| BR[launcher: named browser<br/>or system default]
  L -->|Mobile, device tools present| DV[WASM-012 Run on device<br/>dev mode loads the dev server]
  L -->|Mobile, none| PH[browser at /__prc/phone<br/>same-origin frame at the preset's size]
  W[SourceWatcher] -->|changed| J
  S -->|/__prc/version| U[page script: app.update / reload / overlay]
  U -->|POST /__prc/log| S
  S -->|page events| O[Output panel]
  T[render_form / run_form<br/>tests] --> HH[prc-web-preview<br/>headless only]
```

| Req. | How it is met | State on 2026-10-23 |
|---|---|---|
| **R1** | `WebPreview::start` (`web_preview.rs:607`) builds with `web::backend()` on a thread, the bundle being the Build's own (C16). | Done. |
| **R12, AC11** | §2.1. `WebPreview::serve` (`:756`) no longer takes a `Target` that can be `WebView`: it serves, then calls the **launcher** with the project's `preview_browser`. A named browser that fails to start is a `Note::BrowserFailed { name, why }` and the system default is used (R12's last sentence). `PreviewSession` is no longer started for the interactive Preview. A Preview already serving is **reused**: pressing Preview again re-opens the same address, so a tab the developer has open keeps working. | `Target::Browser` (`:481`) already opens the system browser for Run (`app.rs:2829`); the named browser, the reuse and the removal are new. |
| **R13, AC12** | **In place:** unchanged mechanism (C17), plus the page script measures each `app.update` (`performance.now()` before and after) and POSTs `{"event":"updated","how":<the answer>,"ms":…}`; the IDE prints `Note::Updated { ms, how }` beside the rebuild time it prints today (`:750`). **Relay:** §2.2. **Presets:** none (C10). | Update done; measurement and relay new. |
| **R14, AC13** | §2.3: the IDE asks WASM-012's `mobile_toolchain::detect(platform)` and `mobile_devices::list(platform)`; with a target available it hands the dev server's address to WASM-012's Run on device, whose device log arrives in the Output panel. | Seam new here; device machinery is WASM-012's (`crates/cobolt-compiler/src/mobile_devices.rs`, `mobile_toolchain.rs` — **planned there, not in the tree**). |
| **R15, AC14** | §2.3: no toolchain or no target → `/__prc/phone` in the launcher's browser; `Note::NotTheDevice { platform, install }` in the Output panel and the status line; the page banner says the same. | New. |
| **R16, AC15** | §2.4: the helper has one mode, hidden; `PreviewSession` is used only by the headless runner. A test asserts that no interactive path spawns the helper. | New (the helper today opens a visible window, `cobolt-web-preview/src/main.rs:283`). |
| **R2** | For the interactive Preview, isolation is the browser's and the emulator's (R16). For the headless helper, a dead child is `Event::Closed` (`preview.rs:311` `poll`, `:334` `alive`), as today. | Done. |
| **R3** | See R13. "Keeping the form's state where it can": a stylesheet change keeps every instance and value; a new program module or form description restarts the application in the same player, which is the player's rule (C17), not this plan's. AC2/AC12 count both as in place (no page reload); `how` names which happened. | Done; reported numbers new. |
| **R5** | Console and errors: §2.2. Developer tools: the browser's (R13). COBOL line: the page script reads `app.player.errors()` (`cobolt-web-player/src/api.rs:920`, JSON with `code`, `message`, `form`, `event`, `line`, `text`) after each settled run and POSTs the new entries as `cobol-error`. | New. |
| **R6** | §2.5: the Preview reads the address and the development token from the running `BackendRun`; Run starts the backend first when the project has one (D7). With none running, `Note::BackendOff` (exists) and the A6 `transport` error frame in the page — never silent. | Backend run exists (C18); hand-over to the page new. |
| **R7, AC5** | A failed rebuild leaves `dist/web` as it was (the generator's atomic replace, 2.0.37); the IDE gives the server the error lines in the developer's language (`ServerHandle::set_build_errors`); `/__prc/version` adds `"errors":[…]`, and the page script draws them **over** the page in a dismissable overlay built with `textContent` (D14 of the INDEX); the next good build clears them. The Output panel has them too (exists). | Overlay new. |
| **R8** | For the browser, its own behaviour (spec §5). What stays ours: the launcher opens only `is_preview_url` addresses (`preview.rs:200`); the dev server stays loopback-only with its Host check; the headless helper keeps `may_navigate` (`main.rs:95`). | Done. |
| **R9, AC8** | §2.4, as in the 2026-10-22 plan's §2.3, now the helper's only mode. | New. |
| **R10, AC9** | Every branch stays behind `is_web_project()` (`app.rs:2828`); the desktop Preview window and `ExternalFormRun::spawn` are untouched; `render_form`/`run_form` keep the desktop path for `.cfrm`. | Done; tests kept. |
| **R11, AC10** | The Preview adds no layer state; the form description has no layer `Visible` (`cobolt_forms::model::Layer`); the player starts every layer hidden per instance (WASM-ABI-v1 A9). A restart by `app.update` starts hidden again, as the program would. | Test new (headless). |

## 2. The parts

### 2.1 The launcher (R12, D11)

- `cobolt-project-tools/src/web/preview.rs` gains `open_in_browser(url, choice: &BrowserChoice) -> Result<(), String>` beside `open_in_system_browser`, with the same `is_preview_url` guard and the same `PRC_PREVIEW_NO_BROWSER` test switch (`NO_BROWSER_ENV`, `:47`). `BrowserChoice` is `System`, `Named(KnownBrowser)` or `Path(PathBuf)`.
- `KnownBrowser` is a closed list — Chrome, Chromium, Edge, Firefox, Safari (macOS only), Brave — each resolved per OS: macOS `open -a "<Application name>" <url>`; Windows the executable from the registry's App Paths (`chrome.exe`, `msedge.exe`, `firefox.exe`, `brave.exe`) started with the URL; Linux the first of the browser's usual commands on `PATH` (`google-chrome`/`google-chrome-stable`, `chromium`/`chromium-browser`, `microsoft-edge`, `firefox`, `brave-browser`). A browser not found is an error naming it; the caller falls back to `System` (R12).
- A **path** is started only after the developer confirmed it once on this machine (§2.6). Unconfirmed → `Note::BrowserNotConfirmed` and the system browser.
- Std only (`std::process::Command`); no new dependency.

### 2.2 The page script and `/__prc/log` (R13, R5, R7)

- `devserver.rs` `reload_script` (`:484`) grows into the **Preview script**: today's poll + `app.update`, plus (a) wrapping `console.*`, `window.onerror` and `unhandledrejection` (the helper's init script does this today, `main.rs:104`; one Rust constant is shared so both say the same thing — over `window.ipc` in the helper, over `fetch` in a browser); (b) the `cobol-error` read of §1 R5; (c) the update timing; (d) the error overlay from `/__prc/version`'s `errors`; (e) a "the Preview has stopped" banner when the poll fails three times in a row (the IDE closed the Preview or quit), so a stale tab says so.
- Every event carries `form`: the player's route (`#/form`, WASM-ABI-v1 §12.1) in an SPA, `pr-form` in a standalone window.
- **`POST /__prc/log`** is the server's only non-`GET`/`HEAD` route (today every other method answers 405, `devserver.rs:525`): body ≤ 16 KiB, `Content-Type: application/json`, the `Origin` header equal to the server's own origin (on top of the Host check), at most 40 events per request and 200 per second, events of the closed set `console | error | cobol-error | updated` only. The server parses them into a typed `PageEvent` and queues them for `ServerHandle::take_page_events()`; the IDE turns them into `Note`s (the 40-per-poll cap and the 300-note log are kept).
- Strings the page shows (overlay title and dismiss, the stopped banner, the not-the-device banner) are handed to the server by the IDE in the developer's language and served in the script's data; the page inserts them with `textContent` only.

### 2.3 Mobile: device first, browser as the fallback (R14, R15, D12)

- `WebPreview` for a Mobile kind: `mobile_toolchain::detect(platform)` → if the platform's tools are present and `mobile_devices::list(platform)` names a running or startable target, the dev server's address goes to WASM-012's **Run on device** (development mode loads it: `adb reverse` on Android, the shared loopback on the iOS simulator, WASM-012 D-Dev). Live update is the same page script inside the shell's web view; console relay works the same way, because the page POSTs to the same loopback server; the device log is WASM-012's stream.
- Otherwise → **`/__prc/phone`**: a page the dev server writes on the fly (never on disk), with the same isolation headers, holding a same-origin `<iframe src="/">` sized to the project's `[mobile] device` and `orientation` from `web/presets.rs` (`PRESETS`, `:67`) — CSS pixels, no pixel-ratio emulation — and the not-the-device banner above it. The app inside the frame sees a phone-width viewport, so its CSS media queries behave as on a phone. It opens through the launcher of §2.1, with the project's `preview_browser` (C15).
- `Note::NotTheDevice { platform, install }` names what `detect` found missing (its `MobileTool` rows' next step), and the IDE's status line says "Preview in the browser, not on the device".
- Until WASM-012 lands its two modules, the branch compiles against a small trait in `web_preview.rs` (`DeviceRunner`) whose only implementation answers "no device tools", so every Mobile Preview takes the fallback and AC14 is testable now; AC13 waits for WASM-012.

### 2.4 The headless helper (R9, R16)

- `cobolt-web-preview/src/main.rs`: the window is always created invisible and placed off every screen (`tao` `with_visible(false)` and an off-screen position — the hidden-window risk of §5); `size` as a command, `devtools` and the visible path are deleted; `open` takes the viewport (`width`, `height`, `dpr`, `dark`, `locale`) so `render_form` can ask for a preset and a scheme.
- New commands, unchanged from the 2026-10-22 plan: `render` → `rendered` (a PNG of the viewport through the engine's snapshot); `script` with the closed steps `click`, `type`, `wait`, `get` (+ WASM-015's read-only `text`, `style`, `rect`, `snapshot`, `focused`, `key`) → `scripted`. Steps go through the player's `dispatch`, `type_into`, `invoke` (`api.rs:960`, `:973`, `:951`) and wait on `settled` (`:1008`); there is no `eval`.
- Linux: the `[target.'cfg(any(target_os = "macos", target_os = "windows"))']` gate (`cobolt-web-preview/Cargo.toml:39`) widens to Linux with wry's WebKitGTK 4.1 backend.
- `render_form` / `run_form` (`tools/mod.rs:520`, `:537`) route a `.wcfrm`/`.mcfrm` form to the helper: build once per call, serve, run headless, answer the desktop fields plus `"engine": "webkit" | "webview2" | "webkitgtk"`. No helper → an error saying so, never a desktop picture of a web form.

### 2.5 The backend during Preview and Run (R6, D4, D7, D8)

- `BackendRun` (`backend_run.rs`) already reads the child's lines; it gains `token()` — the line after `development token …` (`cobolt-cli/src/main.rs:587-588`). The token is not printed into the Output panel in clear (it is today, through `BackendEvent::Line`); the Output line reads `<development token>` as the test at `backend_run.rs:199` already prints it.
- On serve, the dev server rewrites `config.json` **for this run only**: the backend's `ws://127.0.0.1:…` address, `unsafe_plain_ws: true` and `backend.dev_token`. `dist/web` on disk is never changed; a test greps it after a run.
- **Gap, owned elsewhere:** the player takes a token only from the Authentication control today (`net.rs:210` `set_token`); it must accept `backend.dev_token` from the config **only** when `unsafe_plain_ws` is set and the address is loopback, and send it as the A8 `auth` message (WASM-016 / WASM-009, `net.rs`).

### 2.6 The setting (Q7, D11)

- `WebSettings` (`project_kind.rs:363`) and `MobileSettings` (`:446`) gain `preview_browser: String`, `#[serde(default, skip_serializing_if = "String::is_empty")]`, so every existing `cobolt.toml` is read and saved unchanged. Empty = the system default; a `KnownBrowser` id (`chrome`, `chromium`, `edge`, `firefox`, `safari`, `brave`); anything else = an application path.
- `panels/web_settings.rs` `show` (`:286`) gains one row: a combo of *System default* and the known browsers present on this machine, plus *Other…* with a path field (the existing `row` layout, no new window).
- **Path confirmation:** the first Preview with a path asks once (*Start "<path>" to show the Preview?*); the answer is kept in the IDE's per-user settings keyed by the project folder and the path, never in the project. A downloaded project cannot make Preview start a program by itself.

## 3. The helper protocol (JSON lines; headless only)

```text
IDE -> helper  {"cmd":"open","url":..,"width":390,"height":844,"dpr":3.0,"dark":false,"locale":"pt-BR"}
               {"cmd":"reload"} {"cmd":"scheme","dark":true} {"cmd":"emulate",..} {"cmd":"close"}
               {"cmd":"render","path":..}   {"cmd":"script","steps":[..]}
helper -> IDE  {"event":"ready"} {"event":"blocked","url":..} {"event":"failed","message":..} {"event":"closed"}
               {"event":"console","level":..,"text":..,"form":..}
               {"event":"error","message":..,"source":..,"line":..,"form":..}
               {"event":"cobol-error","form":..,"unit":..,"line":..,"code":..,"message":..}
               {"event":"rendered",..} {"event":"scripted",..}
```

**Removed:** `size`, `devtools`, `title` on `open`. The page events (`console`, `error`, `cobol-error`, `updated`) have the **same JSON** over `/__prc/log`, parsed by one `PageEvent` type. An old IDE ignores an unknown event; a new helper refuses `size`/`devtools` with `failed`. Each side's tests pin the lines.

## 4. Key decisions & alternatives

- **D1 — Separate process (Q1, INDEX D7 open).** Now moot for the interactive Preview (the browser and the emulator are separate by nature); it still holds for the headless helper. Kept as the working assumption; the INDEX row stays **open** for the operator.
- **D2 — wry 0.57 + tao 0.35 (Q2, decided at the first `/plan`).** Kept for the headless helper; Linux adds wry's `webkit2gtk`/`gtk` bindings (MIT) over the system WebKitGTK 4.1 (LGPL, dynamically linked). Licences re-read with `cargo tree -e normal --target x86_64-unknown-linux-gnu -p cobolt-web-preview` at implementation, recorded in `THIRD_PARTY_NOTICES.md`. The mobile shell uses the same crates through Tauri 2 (WASM-012).
- **D3 — In-place update is the host page's `app.update` (C17).** Rejected: the 2026-10-22 server-side change set (the host page already compares the configs' integrity hashes, which is the same knowledge without a second protocol).
- **D4 — A token reaches the page only through the served, rewritten `config.json`.** Kept. Rejected: the page script or `localStorage` (would outlive the run); `dist/web` (a Build could ship it).
- **D5 — Headless on the same helper (WASM-015 Q1, decided).** Kept.
- **D6 — Update budget (Q5: answered "as short as possible", *to confirm*).** Working assumption, not a decision: a stylesheet change shows within **500 ms** of the save, a program or description change within 500 ms **plus** the build's own time, each printed separately. To get there the page polls at 150 ms while open (today 400 ms, `devserver.rs:499`) and the watcher's look drops to 150 ms with one quiet look. The measured numbers go to the operator.
- **D7 — Preview and Run stay two actions (Q6, not answered).** Working assumption: **Preview** = build, serve, open, live update; offers to start the backend. **Run** = the same, but starts the project's backend first when it has one, so a call works on the first click. Both use the same browser (R12) or device (R14).
- **D8 — Backend during Preview (Q3, not answered).** Working assumption: the real backend run locally (`BackendRun`), no fixture stub.
- **D11 — `preview_browser` in `[web]` and `[mobile]`, a closed list or a confirmed path (Q7, decided here).** Rejected: a free command line (a project file would choose what runs); the setting only in the IDE's preferences (the operator said *project* settings); one `[web]` table for both kinds (a Mobile project has none).
- **D12 — The phone width is a same-origin frame.** Rejected: resizing the browser's window (no portable way for the default browser, and a page cannot resize a tab); a `max-width` on the page (media queries would still see the desktop viewport, so the layout would lie); Chrome's `--window-size` (one browser only).
- **D13 — One POST route for page events.** Rejected: GET with the event in the query (length limits, events in logs and history); a WebSocket on the dev server (a second protocol for one-way traffic); polling the page from the IDE (no channel to a browser tab).
- **D9 — Notes, not strings.** Kept; the controls window it also described is gone.

## 5. Risks & mitigations

- *WASM-012's device modules do not exist yet* → the `DeviceRunner` seam answers "no device tools" so the browser fallback is complete and tested; AC13 is listed as waiting for WASM-012, never claimed.
- *A browser tab outlives the Preview* → the stopped banner (§2.2 e); Preview reuses its server while the project is open.
- *A port the developer bookmarked changes between IDE runs* → stated in the Output line that names the address; not mitigated further (loopback ports are ephemeral by design).
- *`/__prc/log` as a write surface on loopback* → Host and Origin checks, a closed event set, size and rate caps, parsing into typed events (never logged as raw bytes); another local process could still POST — it can only add lines to the Output panel, which is stated.
- *The frame's isolation* → the phone page is served with the bundle's COOP/COEP, and a same-origin frame keeps cross-origin isolation, so the threaded player still runs; a test reads `crossOriginIsolated` inside the frame headless.
- *A hidden window does not paint* (WebView2 suspends an invisible window) → off-screen placement instead of hiding; an engine that still refuses gets an error naming it, never a fake picture.
- *Linux CI has no display* → the headless runner needs Xvfb or a headless Wayland compositor; helper tests print why and skip without one; CI sets `PRC_REQUIRE_WEBVIEW=1` (WASM-015 plan) so a skip there fails.
- *The player does not take a development token yet* → R6 calls answer the `authentication` error frame until WASM-016/009 add the intake; the Output panel says so.
- *Windows untested on this machine* → the CI matrix builds and runs the helper and the launcher tests there; a real browser window is the operator's to look at.

## 6. Affected crates / files

- `crates/cobolt-project-tools/src/web/devserver.rs` — the Preview script (§2.2), `POST /__prc/log`, `PageEvent`, `take_page_events`, `set_build_errors` and `errors` in `/__prc/version`, the served strings, `/__prc/phone`, the per-run `config.json` rewrite.
- `crates/cobolt-project-tools/src/web/preview.rs` — `open_in_browser`, `BrowserChoice`, `KnownBrowser`; `Mode::WebView` and the visible spawn removed; `PreviewSession` kept for the headless runner; new commands/events (§3).
- `crates/cobolt-project-tools/src/tools/mod.rs` — the web route of `render_form`/`run_form`.
- `crates/cobolt-project-tools/src/content.rs` — `:451` and `:1203-1204` say a web form is previewed in the browser and on the emulator, and that `render_form`/`run_form` use the system web view headless.
- `crates/cobolt-web-preview/{Cargo.toml,src/main.rs}` — Linux target, headless-only window, `render`, `script`, `cobol-error`, `form`; `size`, `devtools` removed.
- `crates/cobolt-compiler/src/project_kind.rs` — `preview_browser` on `WebSettings` and `MobileSettings`, with round-trip tests that an existing manifest saves byte-for-byte.
- `crates/cobolt-ide/src/panels/web_preview.rs` — `Target::WebView` and the controls window (`show`, `:887`; `Controls`) removed; the launcher; server reuse; `DeviceRunner` and the Mobile branch; `Note::Updated { ms, how }`, `BrowserFailed`, `BrowserNotConfirmed`, `NotTheDevice`, `CobolError`, `PreviewStopped`.
- `crates/cobolt-ide/src/panels/web_settings.rs` — the browser row.
- `crates/cobolt-ide/src/app.rs` — `toggle_web_preview` (`:2736`) and `do_run` (`:2823`) take the browser/device route; Run starts the backend first (D7); the path confirmation; the Preview stops with the project.
- `crates/cobolt-ide/src/backend_run.rs` — `token()`, and the token kept out of the Output panel.
- `crates/cobolt-ide/src/i18n.rs` — new keys in **six languages** (§8); the `pk_prev_*`/`pk_ps_*` keys whose only reader was the controls window are removed with it.
- `.github/workflows/build-all-platforms.yml` — build and package `prc-web-preview` on all three systems; `.deb`/`.rpm` gain WebKitGTK 4.1; the Windows text names WebView2 — now stated as needed for **agent and test renders** only.
- The IDE's toolchain check (`crates/cobolt-ide/src/toolchain.rs`) — a missing WebKitGTK/WebView2 is reported as "render_form/run_form cannot draw web forms", not as a Preview or Build blocker.
- `THIRD_PARTY_NOTICES.md` — the Linux crates of D2.
- **Not here:** the device modules (WASM-012), the player's dev-token intake (WASM-016/009). **Never:** `README.md`, `docs/` (2.x.x rule).

## 7. Test strategy

Every test prints what it measured; none asserts a number it did not produce. Browser launches use `PRC_PREVIEW_NO_BROWSER` and assert the **decision** (which browser, which address); the page itself is driven headless.

| AC | Test (crate) | Asserts and prints |
|---|---|---|
| AC11 | `cobolt-project-tools` launcher tests + `cobolt-ide --bin` flow on a hello-world Web project | with no setting the system browser is chosen, with `firefox` that one, with a missing one the system browser and `BrowserFailed`; no helper process is spawned; the served page, driven headless, changes its label on a click |
| AC12 | flow + headless page | a property, a class rule and the stylesheet each arrive as `updated` with `how` ≠ `reload` and the page not reloaded; ms printed per kind against D6; a `console.error` and a thrown error reach the Output notes with the form name; a COBOL run-time error arrives with unit and line |
| AC13 | waits for WASM-012; a `DeviceRunner` fake proves the hand-over (the dev address is passed, the device log lines reach the Output notes) | the fake's calls in order |
| AC14 | flow on a Mobile project with the "no tools" runner | `/__prc/phone` is opened; the frame's `innerWidth` read headless equals the preset's width; the banner text and `NotTheDevice` name the missing tool, in six languages |
| AC15 / AC8 | `cobolt-project-tools/tests/tools.rs` web cases | `render_form`/`run_form` on a web form answer the desktop fields plus `engine`; five renders of the reference form within per-channel ≤ 2 and ≤ 0.5 % of pixels, the difference printed; the helper reports its window invisible |
| AC5 | flow with a backend failing on the second build | the bundle is still the first; `/__prc/version` carries the errors; the overlay's text read headless |
| AC9 | the existing desktop Preview / Run Form / `render_form` tests | green; no web action for a Desktop project |
| AC10 | headless layered web form | each layer's visibility `false` at open and after an `app.update` restart; `true` only after the program's `SET … Visible TO TRUE` |
| R6 | flow with a fixture `backend/` through `BackendRun` | the served `config.json` names the backend and a token; the Output panel shows `<development token>`; `dist/web` contains no token afterwards |
| `/__prc/log` | `devserver.rs` unit tests | wrong Origin, wrong Host, oversize, unknown event, burst over the cap: each refused, counted, printed |
| Q7 | `project_kind.rs` round trip | an existing Web and Mobile `cobolt.toml` saves unchanged; a set value round-trips; an unconfirmed path is not started |

Helper-dependent tests skip with a printed reason when the helper or a display is absent. Sweeps use `--no-fail-fast` and read every `test result` line (`cobolt-ide --bin cobolt-ide`).

**Manual (the operator's, never an agent's):** Preview a Web project in the default and in a named browser, edit the stylesheet and watch it change, break the build and see the overlay, close the IDE and see the stopped banner; Preview a Mobile project with and without an emulator. Agents use the headless helper and the MCP tools, never the IDE window.

## 8. Steering compliance

- [ ] **i18n:** every new string a `Tr` key in all six languages: the browser row and its choices, the path confirmation, `BrowserFailed`, `BrowserNotConfirmed`, `NotTheDevice` and the status line, `Updated` per `how`, the COBOL-error line, the overlay title and dismiss, the stopped banner, the headless "no web view" error. Page-side strings are served in the IDE's language.
- [ ] **Generated-code contract:** unchanged; the Preview consumes the Build's output. The Preview script, `/__prc/phone` and the `config.json` rewrite are served, never written to disk, so they carry no banner.
- [ ] **Interpreter/binary parity:** no interpreter change; the backend is `rcrun backend run`, the same server code as the built backend.
- [ ] **System KB:** no control, property, method or event changes; `content.rs` texts change — if `prebuilt_chunked_kb_matches_the_published_documentation` turns red, regenerate `assets/knowledge/chunked.data` with `cargo run -p cobolt-ide --example build_chunked_kb` in the same change.
- [ ] **PRIME DIRECTIVE:** the page scripts and the phone page are Rust string constants in `devserver.rs` and `cobolt-web-preview/src/main.rs`; no `.js` or `.html` file is added.
- [ ] **User code is sacred:** the Preview never writes into the project's sources; `preview_browser` is written only when the developer sets it.
- [ ] **Public formats:** `cobolt.toml` gains one optional key in two tables, absent unless set.
- [ ] **Self-resizing windows:** no new window; the controls window is removed; the settings row uses the existing layout.
- [ ] **Docs:** `README.md` and `docs/` untouched (2.x.x).
- [ ] **Versioning:** feature on `features-2x`; each change bumps `z` with a `CHANGELOG.md` entry (bookkeeping by the pipeline).
- [ ] **No "cobolt" in user-facing text**; the helper's binary is `prc-web-preview`.

## 9. Open questions

- **Q1** (separate process) — moot for the interactive Preview; working assumption D1 for the helper; INDEX D7 still **open**.
- **Q3** (backend during Preview) — **not answered**; working assumption D8.
- **Q5** (update budget) — answered "as short as possible", **to confirm**; working number D6 until the operator gives one.
- **Q6** (Preview vs Run) — **not answered**; working assumption D7 (Run starts the backend first).
- **Decided:** Q2 (wry + tao, first `/plan`); Q4 (operator: a development token, D4, §2.5); **Q7 at this `/plan`** (D11, C15) — the operator may overrule.
- **Dependencies, not questions:** WASM-012's `mobile_toolchain` and `mobile_devices` (AC13); the player's development-token intake (WASM-016/009).
