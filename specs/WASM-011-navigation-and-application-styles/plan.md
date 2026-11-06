<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Navigation and application styles (WASM-011)

- **Status:** implemented 2026-11-01 (2.0.127) except where named. **Built and tested:** R1 to the bundle, R2, R3 on the web (the Build's shared check and the player's run-time refusal, no manifest field: the description carries the format), R4/R5 with an SPA without a shell, one address owner per page (Q7), R6 (not found; the sign-in hook is WASM-009's gate), R7, R8, R10's event, R12's `pagehide` close, R13's `stack` flag, R16 (drawn by WASM-004). **Not built:** R11's suspension of `super::` (T17: the generator's suspend point, WASM-010 phase C), same-window standalone (T22: PQ1, PQ2 not answered). **Browser runs owed:** AC2–AC5, AC8, AC9, AC1's `pr_dbg_live` half, AC6's sign-in half, two applications on one page (§6). PQ1–PQ4 are still the operator's; the names in §8 are working assumptions. The plan's ABI rows "A10"/"A11" were taken by WASM-010 meanwhile; this spec's row is **A20**, and A10's manifest field was not needed.
- **Spec:** ./spec.md   **Date:** 2026-10-22 (first version 2026-10-20)

## 0. What changed in this revision, and why

| Change | Why |
|---|---|
| Q1, Q2, Q3, Q5, Q7 are now **operator decisions** and the plan follows them; Q2's "path as an option" is dropped | Q2 was decided as **hash**, with no path option. Q1 (form and parameters only, never data) is what the player already does. Q3 confirms the one-origin, one-profile limit and that a closed opener is a normal `ON EXCEPTION` |
| A **same-window** variant of the standalone style is added (§1.8) | Q4: same-window multi-page navigation **is part of the standalone style**. How the developer picks it, and what `OpenFormSync` does there, are not decided: **PQ1** and **PQ2** in §8 |
| Mobile: **SPA with a stack** by default (§1.10) | Q5. R13's stack is used for the SPA's own navigation on a phone too, not only for standalone screens |
| **Several applications on one page** (§1.9) | Q7: allowed. The player today assumes one application per page: the address is global (`sync_address`, `follow_address`, `watch_address`) and the panic hook is a page-global `thread_local!` |
| R16 / AC11, **layers** (§1.11) | Added to the spec on 2026-10-21 (commit `fa0c844b`). The manifest has no `layers` yet (ABI A9 is written but `cobolt-wasm-abi::manifest::FormEntry` has no such field) |
| R1 is recorded as **built** (§1.1) | `cobolt-compiler::project_kind::ApplicationStyle`, the New Project choice and the settings panel exist; the earlier plan listed them as "not here" |
| R3 is given a web design (§1.2) | The desktop check exists in `cobolt-semantic`; the player's `form_verb` does not know `FormFormat` |
| R11's `super::` **suspension** is now planned, not deferred (§1.7) | The spec still requires "synchronous from the program's (the caller is suspended meanwhile)"; Q3's answer did not lift it. The replica stays as a cache, not as the answer |
| The player's own **texts** go through a six-language table (§1.12) | The popup notice and the unknown-form notice are English literals (`dom/ops.rs` `open_window`, `open_named`); the spec's i18n line names them |
| The System KB obligation is spelled out (§2, §7) | The spec's constraint names `OpenFormSync`/`OpenFormAsync`, `super` and the new events; no KB text describes the web behaviour today |

## 1. Approach

The opening verbs are the host's (`crates/cobolt-web-player/src/host.rs` `Host::form_verb`); what a verb does depends on `HostOptions.standalone`, which the player sets from `PlayerConfig.standalone` (`src/api.rs`), which the loader sets from `config.style` (`crates/cobolt-web-bundle/assets/loader.js`: `standalone: config.style === 'standalone'`), which the bundle writes from `BundleOptions.style: AppStyle` (`crates/cobolt-web-bundle/src/lib.rs`).

### 1.1 R1 — the style is a project setting (built)

- `crates/cobolt-compiler/src/project_kind.rs` `ApplicationStyle { Spa (default), Standalone }` is `WebSettings.style` and `MobileSettings.style` (`[web]` / `[mobile]` in `cobolt.toml`).
- The New Project choice: `crates/cobolt-project-tools/src/create.rs` (`web.style` / `mobile.style` from `resolved.choice("style")`, default SPA).
- Changeable before a Build: `crates/cobolt-ide/src/panels/web_settings.rs` (`style_text`, the choice loop over both styles; Tr keys `pk_c_spa`, `pk_c_standalone` in all six languages of `crates/cobolt-ide/src/i18n.rs`).
- **Open end, not this spec's to build:** the real Web build backend is not linked yet — `crates/cobolt-project-tools/src/web/mod.rs` falls back to `NotYetLinked`, and the only implementations of `WebBuildBackend` are test stubs (`tests/kinds.rs`, `cobolt-ide/src/panels/web_preview.rs`). `AppStyle` is set only by `cobolt-web-bundle/examples/fixture_bundle.rs`. When WASM-002's backend is linked it must map `WebBuildRequest.web.style` → `BundleOptions.style`; this plan adds the test that pins the mapping (§6) and leaves the backend to WASM-002.

### 1.2 R3 — `FormFormat` on the web

- **Build time (literal targets):** the desktop check is `crates/cobolt-semantic/src/resolver.rs` `check_open_form_target` (049 R17: `OpenFormSync`/`OpenFormAsync`/`OpenStandAloneForm*` refuse an `Embedded` target), fed by the form map that `crates/cobolt-compiler/src/lib.rs` builds from each `.cfrm`'s `form_format`. The Web build runs the same semantic pass with the same map — **one check, both kinds**; nothing is copied. The menu side ("a menu item may load only a form that allows `Embedded`", the KB text in `cobolt-compiler/src/lib.rs`) applies unchanged to the SPA's SideMenu.
- **R3's second sentence** needs no code: in an SPA a `Standalone` or `Both` form opened with `OpenForm*` is shown as `OpenMode::Modal` / `OpenMode::Panel`, never `OpenMode::Embedded`, so it passes the check as it does on the desktop. A test pins it.
- **Run time (data-item targets):** the semantic pass leaves a dynamic target "a runtime concern". The player cannot check it: `FormEntry` has no format. **Additive ABI amendment A10:** `forms[i].format` = `"standalone" | "embedded" | "both"`, `serde(default)` = `"both"` (an older manifest refuses nothing). `form_verb` refuses an `Embedded` target with `CmdError::Bad` (the program's `ON EXCEPTION`); `resolve_menu_item`'s embed path refuses a `Standalone` one.

### 1.3 R2 — the verbs in each style (built)

- **SPA:** `form_verb` queues `Work::Open` with `OpenMode::Modal` (`OpenFormSync`) or `OpenMode::Panel` (`OpenFormAsync`); a SideMenu load opens `OpenMode::Embedded` (`dom/ops.rs` `resolve_menu_item`). `OpenFormSync` sets `Wait::Child` on the in-flight task (ABI A5: tasks of different instances interleave).
- **Standalone (new tab):** `form_verb` emits `Effect::OpenWindow`; `dom/ops.rs` `open_window` opens `?pr-form=Name&pr-opener=H&pr-super=T&pr-param-<k>=<v>…` with `window.open(…, "_blank")`; `watch_window` polls `closed` every 500 ms and pushes `Inbox::ChildClosed`, which releases a `Wait::Child`. The same-window variant is §1.8.

### 1.4 R4, R5 — routes (built), Q1 and Q2 applied

- `crates/cobolt-web-player/src/view/shell.rs` `Route { form, params }`, `to_hash` / `parse`: `#/form-name?k=v` (percent-encoded). **Hash only** (Q2, decided): no path mode is built, so no host needs a rewrite rule.
- `dom/instance.rs` `sync_address` pushes the route of the top of the chain (none for the main form); `dom/ops.rs` `follow_address` follows Back / Forward / a pasted address; `watch_address` listens to `popstate` (the player's own `pushState` does not fire it, so no loops). A hash that is not a route (`#section`) is the page's own (R5).
- **Q1 (decided):** a reload or deep link restores **the form and its opening parameters**, never in-memory data, nothing in `sessionStorage`. That is what `follow_address` does (`open_named(…, route.params)`); `GetParam` / `GetParams` answer the parameters already in `onLoad` (ABI A7 §12.1). Parameters from an address are left out when bad (`host.rs` `sanitize_params`; a program's go through `parse_params`; both use `param_problem`; limits `MAX_PARAMS` 16, `MAX_PARAM_NAME` 64, `MAX_PARAM_VALUE` 512); a bad one in a program's open refuses the open.
- **The data region** (R4's last sentence, WASM-017 R2/R5) is the generator's: `pr_open_form` allocates it, `pr_close_form` releases it; the player calls `pr_close_form` only when an instance is destroyed (`Effect::Closed`), never for a resident or `PreservePreviousForm` form (`dom/instance.rs` `preserve_next`). No change here; AC1's test asserts it with `pr_dbg_live` (ABI A3).
- **Gap:** `follow_address` returns early unless the shell is on (`shell_on`), so an SPA **without** a SideMenu does not follow a pasted route, and `sync_address` still pushes one for its modal / panel. Fix: an SPA without a shell follows a route by opening the named form as a `Panel` over the main form (the R2 shape for a form the main form opened), so a pushed route can always be followed back.

### 1.5 R6 — not found, sign-in, and the event

- Today `open_named` shows the English notice "The application has no form named …" and nothing else.
- New: an address that names no form, or a form the application may not open for this user, gives a **defined outcome**: the main form stays shown, a not-found notice (six languages, §1.12) is shown, the route is **replaced** (`replaceState`, so Back does not return to it), and an event is raised on the **main form** with the route's text as its argument (ABI A7 §12.2 gets the row). The event's name is **PQ3** (working assumption: `onRouteNotFound`).
- **Sign-in** (WASM-009 R11): when a route names a form that requires sign-in and there is no session, `follow_address` hands the route to the Authentication gate instead of opening it, and the gate re-follows it after sign-in. The gate is WASM-009's (its Q4 "which forms require sign-in" and R11); this spec only adds the hook in `follow_address` and AC6's second half waits for it.

### 1.6 R7, R8, R14 — accessibility, unsaved changes, modals

- **R7:** on every `Effect::Opened` for `Embedded`, `Root`, `Panel` or a standalone start form, the player (a) focuses the form's title element (made focusable with `tabindex="-1"`), (b) writes the form's title into one polite `aria-live` region owned by the player's root (`dom/mod.rs`, next to the existing hosts), (c) scrolls the content element to the top. A modal focuses its own title rather than its first field. The announced text is the form's own title — the developer's — so no new strings.
- **R8:** a form marks itself with a form verb (**PQ4**, working assumption `me::SetUnsavedChanges(1|0)`; the runtime-properties rule forbids inventing a property without the desktop, so the desktop interpreter must accept the verb as a no-op — R15 holds, nothing else changes there). While any shown instance is marked: closing the tab → `beforeunload` (the browser's own text; browsers show no custom text); Back → `popstate` cannot be cancelled, so the player re-pushes the route it is leaving and asks an in-page confirm (six languages), then goes back or stays; a menu load → the same confirm before the load.
- **R14 (built):** a modal is a `<dialog>` with `showModal()` (inert background, focus trap, Escape routed to the form's close through `cancel`, stacking in opening order).

### 1.7 R9–R12 — standalone

- **R9 (built):** each window runs its own instance (its own module instance, its own data). **Q6 (decided):** every page loads the project's one program module and the one player, started with the page's form (`PlayerConfig.start_form` from `pr-form` or `data-pr-start`); the bundle also writes `<Form>.html` per form for Standalone (`cobolt-web-bundle/src/lib.rs`, the `AppStyle::Standalone` loop) for bookmarks.
- **R10:** built — a refused `window.open` shows the notice and pushes `Inbox::ChildClosed`, so the suspended caller is released with a failure. To add: the notice text through §1.12, and an event on the opener's form (**PQ3**, working assumption `onPopupBlocked`, argument the form name).
- **R11, `super::` across windows (built, replica form):** `src/superlink.rs` — a `BroadcastChannel` private to the opener-and-child pair (128-bit token in `pr-super`); `Host::{attach_remote_opener, remote_opener_state, apply_super_set, apply_super_call}`; `Effect::{SuperSet, SuperCall}`; the opener publishes its form's properties after each change and announces `closed` on `pagehide`. **Q3 (decided):** the same-origin, same-profile limit is accepted; a closed opener is a normal `ON EXCEPTION` — the host's refusal already is a `CmdError`, and AC5's test must use a fixture **with** `ON EXCEPTION` (the 2026-10-20 run's fixture had none, so it trapped).
- **R11, the suspension still owed:** the spec requires the caller to be suspended while a `super::` read, write or call crosses windows. Design: a request `{"type":"get"|"set"|"call","id":N,…}` answered `{"id":N,"value":…}` or `{"id":N,"error":…}`; a new `Wait::Super { id, done }` beside `Wait::Child` in `host.rs`; a timeout (the remote-call timeout, `PlayerConfig.call_timeout_ms`) and `closed` both answer the error → `ON EXCEPTION`. Queries (ABI §6) are synchronous, so the module cannot wait inside a `GET`: **ABI amendment A11** — a `GET`/`CALLR` on ctrl `1` in a window with a remote opener answers a new code "suspended, re-issue after resume"; the module returns `SUSPENDED`, and on `pr_resume` re-issues the query, which the host now answers from the reply. This affects the generator (WASM-010 phase C, where suspension lives) and is the reason it was deferred; the replica stays as the source a re-issued query reads.
- **R12:** built — a closed window releases a `Wait::Child`. To add: the child window runs its closing handlers on `pagehide` (`pr_close_form`, as far as the browser lets a page run while it goes away; a handler that suspends there is abandoned with `pr_cancel`), and the opener's `onClose`-side events for that handle run when `ChildClosed` arrives.

### 1.8 Q4 — same-window navigation inside the standalone style (new)

Q4 is decided: a classic multi-page site (each `OpenForm` replaces the page) **is part of the standalone style**. What the developer chooses and how `OpenFormSync` behaves when the opener's page is gone are not decided (PQ1, PQ2). Working assumption, built only after the operator confirms:
- A sub-option of the standalone style, `[web] standalone_window = "new" | "same"` (default `"new"`, the behaviour of today), shown in `web_settings.rs` only when Standalone is chosen (Tr keys ×6).
- `"same"`: `OpenFormAsync` does `location.assign` to the form's address (`?pr-form=…&pr-param-…`, no `pr-super`): the opener's instance ends (its closing handlers run on `pagehide`, §1.7), Back reloads the opener's page from its own address and parameters (Q1: no data). `super::` has no opener and fails with `ON EXCEPTION`.
- `OpenFormSync` cannot suspend a caller whose page is unloaded; working assumption (PQ2): the Build reports it as an error in a same-window project, naming the statement.

### 1.9 Q7 — several applications on one page (new)

Allowed (decided). Each `Player` is mounted on its own element (`api.rs` `Player::mount`; the bundle's `Embed::Element { selector }`). What has to change:
- **The address has one owner.** The application mounted with `Embed::Window` owns the hash; an `Embed::Element` application does **not** touch the address unless its configuration says `address: "own"` (new, in `options.embed`), and at most one on a page may (the second is refused at mount with a message). A non-owner keeps its route chain in memory (Back inside it is its breadcrumb). This also satisfies R5's "shall not hijack the host page's address when embedded"; today `sync_address` pushes for every non-standalone application. The bundle's existing note for Element + Standalone (`lib.rs`) stays.
- **No page-global state per application:** the panic hook (`api.rs` `PANIC_FN` `thread_local!`) and `dom/apply.rs` `FOCUS` are per wasm instance; a task verifies whether two loaders on one page share one player wasm instance, and if they do, keys the panic hook by application.
- The `BroadcastChannel` tokens are already per opener-and-child pair, so two applications of the same origin do not hear each other.

### 1.10 R13, Q5 — mobile

Q5 (decided): the mobile default is **SPA with a stack** — `MobileSettings.style` already defaults to SPA. In the mobile shell (WASM-012, Tauri 2) the SPA's chain is the stack: a SideMenu load, `OpenFormAsync` and `OpenFormSync` push a full-screen screen; the system Back pops the top one (through the same `popstate` path, since the shell's web view keeps history; `OpenFormSync`'s caller is released when its screen is popped). A standalone Mobile project pushes screens the same way (R13). A deep link of the OS's scheme is turned into the hash route by the shell and followed by `follow_address`. The shell is WASM-012's; this spec adds a `PlayerConfig` flag `stack: bool` that makes `Panel` / `Modal` render full-screen, and the tests of §6 run with it.

### 1.11 R16 — layers in a pane or a window (new)

Layers are WASM-004 R13–R24 and ABI A9; their drawing is WASM-004's. WASM-011's share:
- A form's layers are drawn **inside the instance's root** (`dom/instance.rs`, the `section.pr-form` element), clipped to the area the form occupies in the ContentPane or the standalone page — never in the page-level `pr-layer` host that `dom/mod.rs` creates for the player's overlays.
- **Routing never shows a layer:** a route opens a fresh instance whose layers are all `Visible = false` (A9: "false when `pr_open_form` creates the instance"); a route with the same parameters re-uses the shown instance and leaves its layers as they are. `Route` carries no layer state.
- Blocked until `cobolt-wasm-abi::manifest::FormEntry` carries `layers` (A9); AC11 is tested when it does.

### 1.12 The player's own texts (i18n)

The loader already merges `config.texts` over `DEFAULT_TEXTS` for its own panels, and `BundleOptions.texts` writes them. The player's texts (the popup notice, not found, the unsaved-changes confirm) get the same route: a six-language default table in the player (Rust, keyed by `PlayerConfig.locale`: en, es, pt-BR, fr, ja, zh), overridable by `config.texts` keys (`popup_blocked`, `route_not_found`, `leave_unsaved`, `leave`, `stay`). The application's own texts stay the developer's.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/host.rs` — `form_verb` (A10 format check), `Wait::Super`, the not-found / popup-blocked events, `SetUnsavedChanges`; tests in `src/host/{params,super}_tests.rs` and a new `src/host/nav_tests.rs`.
- `crates/cobolt-web-player/src/dom/ops.rs` — `follow_address` (no-shell SPA, not found with `replaceState`, sign-in hook, unsaved confirm, address owner), `open_window` (texts, event, same-window), `open_named`.
- `crates/cobolt-web-player/src/dom/instance.rs` — `sync_address` (owner only), focus / live region / scroll on open, layers inside the instance root.
- `crates/cobolt-web-player/src/dom/mod.rs` — the live region; `src/superlink.rs` — the request / reply messages; `src/api.rs` — `PlayerConfig` `texts`, `stack`, `address`, the per-application panic hook; a new `src/texts.rs` (the six-language table).
- `crates/cobolt-wasm-abi/src/manifest.rs` — `FormEntry.format` (A10, `serde(default)`); `layers` comes with WASM-004.
- `crates/cobolt-web-bundle/src/lib.rs` — `format` in the manifest's forms, `options.embed.address`, the player texts in `texts`; `assets/loader.js` — pass `texts`, `stack`, `address` through. (`loader.js` is a tracked file of the product; every new rule is written in the Rust player, the loader only forwards configuration.)
- `crates/cobolt-semantic/src/resolver.rs` — unchanged rule; a Web-project test. If PQ2's assumption is confirmed: the same-window `OpenFormSync` error there.
- `crates/cobolt-compiler/src/project_kind.rs` — `standalone_window` (only if PQ1 is confirmed); `crates/cobolt-ide/src/panels/web_settings.rs` + `crates/cobolt-ide/src/i18n.rs` — its choice, Tr keys ×6.
- `crates/cobolt-runtime` (interpreter) and the compiled binary's host — accept `SetUnsavedChanges` as a no-op (interpreter/binary parity).
- `specs/WASM-ABI-v1.md` — A10 (`format`), A11 (suspended `super::` query), §12.1 (`SetUnsavedChanges`), §12.2 (the two events' argument text).
- **System KB:** the documentation tables in `crates/cobolt-compiler/src/lib.rs` (the section that explains `OpenFormSync` / `OpenFormAsync`, `FormFormat` and `super`) get the web behaviour, the new verb and events; `assets/knowledge/chunked.data` is rebuilt in the same change.
- Not touched: `README.md`, `docs/` (2.x.x); the desktop window model (`cobolt-form-host`).

## 3. Data / model changes

- `.cfrm`: none.
- `cobolt.toml`: `[web] style` exists; `[web] standalone_window` only if PQ1 is confirmed (`serde(default)` → today's behaviour).
- Manifest: `forms[i].format` (A10, default `"both"`); `forms[i].layers` (A9, WASM-004).
- Bundle `config.json`: `options.embed.address`, `options.stack`, player keys in `texts` — all optional, absent = today's behaviour.
- The address grammar (`#/form?k=v`, `?pr-form=…&pr-opener=…&pr-super=…&pr-param-k=v`) is public once shipped; its limits are `host::{MAX_PARAMS, MAX_PARAM_NAME, MAX_PARAM_VALUE}`.
- `superlink` messages: the `get`/`set`/`call` requests gain an `id` and a reply; an older peer that sends none is answered from the replica as today.

## 4. Key decisions & alternatives

- **A hash route, no path option** (Q2, operator). Rejected: path routing (needs server rewrites).
- **Restore the form and its parameters only** (Q1, operator). Rejected: a `sessionStorage` snapshot of data.
- **One `FormFormat` check for desktop and web**, in `cobolt-semantic`, plus a run-time refusal for dynamic targets from the manifest. Rejected: a web-only copy of the rule.
- **Suspend `super::` through a re-issued query (A11)**, keeping the replica as the data it reads. Rejected: the replica as the answer (the spec requires the caller to wait and to see `ON EXCEPTION` when the opener is gone); a blocking wait inside a query (the module cannot block on another window).
- **A private channel per opener-and-child pair.** Rejected: one channel per application (any page of the origin could listen).
- **One address owner per page** (Q7). Rejected: a namespaced hash per application (two applications rewriting one hash still fight over Back).
- **Not-found replaces the route** rather than pushing it, so Back leaves the bad address behind.
- **Player texts in Rust, six languages, overridable by `config.texts`**, the loader's existing pattern. Rejected: English literals (today), or texts in the loader's JavaScript.

## 5. Risks & mitigations

- Risk: A11 changes the generator. → It is additive (only ctrl `1` in a window with a remote opener); until the generator emits it, the replica answers as today and the plan's status says so.
- Risk: `beforeunload` and `pagehide` are best-effort (mobile browsers may skip them). → R8 and R12 say "as far as the browser allows"; the tests assert what Chromium does and the ledger names what is not guaranteed.
- Risk: the popup blocker differs per browser. → AC4 is run in Chromium and Firefox headless with popups blocked; a refusal always releases the caller (already built).
- Risk: two applications on one page share a player wasm instance. → §1.9's verification task; the panic hook is keyed by application if so.
- Risk: a hash that is not a route. → Left to the page (built).
- Risk: same-window navigation loses the opener's state. → It is the decided Q1 behaviour (form and parameters only); the KB text says so.

## 6. Test strategy

- **Native (cargo, `cobolt-web-player --lib`):** existing `params_tests` (grammar, limits, `GetParam` in `onLoad`, bad address parameters left out), `super_tests` (6 tests), `shell` `Route` tests, `tests/abi_host.rs`. New `nav_tests`: an `Embedded` target refused by `form_verb` from `format`, a manifest without `format` refusing nothing; a `Both` form opened by `OpenFormSync` in an SPA opens `OpenMode::Modal` (R3 second sentence); the not-found event raised with the route text; the popup-blocked event raised and `Wait::Child` released; `Wait::Super` resumed by a reply, by a timeout and by `closed`, each ending in the expected `ON EXCEPTION` path; `SetUnsavedChanges` sets the mark; the texts table has every key in all six languages (the test reports any missing key by language).
- **`cobolt-semantic`:** a Web project's `OpenFormSync("EMBEDDED-FORM")` is the same 049 R17 error as on the desktop.
- **`cobolt-web-bundle`:** `format` in the manifest from the `.cfrm`; `embed.address`; a pinned mapping `ApplicationStyle` → `AppStyle` (a helper both the fixture and the future backend use).
- **`cobolt-ide`:** `web_settings` round-trips the style (exists); the `standalone_window` choice if PQ1 is confirmed. Every new Tr key in six languages, a frame drawn in each language.
- **Desktop unchanged (R15, AC10):** `cargo test -p cobolt-semantic -p cobolt-compiler -p cobolt-form-host` green; the interpreter test that `SetUnsavedChanges` is a no-op.
- **Headless Chrome (and Firefox for AC4), through the fixture bundles of `cobolt-web-bundle/examples/fixture_bundle.rs`:** AC1 (three SideMenu forms, address changes, Back, reload, `pr_dbg_live` before/after); AC2 (statement after `OpenFormSync` not run until close); AC3 (new tab seen by the browser, caller resumed at close); AC4 (timer-driven open with popups blocked → failure + notice + event); AC5 (fixture with `ON EXCEPTION`: write/read/call through `super::`, then the opener closed → the handler's `ON EXCEPTION` branch runs); AC6 (unknown route → notice, event, route replaced; the sign-in half when WASM-009 R11 lands); AC7 (axe-core-style check of focus on the title and the live region — run by the browser tooling, not a file in the repository); AC8 (Back and tab close prompt); AC9 with `stack: true` (and in the WASM-012 shell when it exists); AC11 when A9 is in the manifest; two applications on one page (only the owner writes the hash). Each run reports the browser version, the bundle and what was seen.
- **Not runnable here yet, stated plainly:** the mobile shell (WASM-012), the sign-in outcome (WASM-009 R11), the IDE Build path (WASM-002's backend not linked).

### 6.1 Runs recorded (2026-11-01)

| Run | Browser | Bundle | Seen |
|---|---|---|---|
| `cobolt-web-bundle/tests/navigation_chrome.rs` | Chrome 156.0.8078.80, headless | the fixture program, plain, `Isolation::RequireCorp`, served by `devserver` | start: live region `main-form`, focus on the main form's root, address owned; `#/child-form` typed: opened over the main form (no shell), announced, its title focused; Back: closed, hash empty; `#/no-such-form`: only the main form, the English notice, the address taken back |
| `cobolt-web-bundle/tests/page_layers.rs` | the same | the fixture's main form with two layers | the root clips; each layer inside it, a stacking context after the base, hidden and inert at start (R16) |

Owed on 2026-11-01 (T25), since run: AC1, AC2, AC3, AC4 (Chromium), AC5, AC7, AC8, AC11 and the two-applications page — see §6.2. Still owed: see the end of §6.2.

### 6.2 Runs recorded (2026-11-06, WASM-011 T25)

Every run is a Rust test in `crates/cobolt-web-bundle/tests/` (nothing outside the tree): the application is a project written in code (`tests/support/project.rs`), built by the product's own Web Build (`GeneratorBackend`, the same generator the IDE's Build calls), served by `devserver` with `Isolation::RequireCorp`, and clicked by headless Chrome over CDP (`tests/support/cdp.rs`). The browser is Google Chrome 157, headless (its user agent reports `HeadlessChrome/157.0.0.0`: the build number is reduced).

| AC | Test | Bundle | Seen |
|---|---|---|---|
| AC1 | `acceptance_spa_chrome.rs` `ac1_…` | SPA, a SideMenu with three entries (`orders-form`, `customers-form`, `reports-form`) | Orders → `#/orders-form`; Customers → `#/customers-form` and Orders torn down; Back → `#/orders-form`; a reload at `#/reports-form` reopens it and its `onLoad` runs |
| AC2 | `ac2_…` | SPA, a form with `OpenFormSync` and `OpenFormAsync` buttons | `OpenFormSync`: a modal `<dialog>` (`:modal`), the caption written after the verb unchanged for 500 ms, `after sync` when the dialog's `Close` ran; `OpenFormAsync`: a modeless panel and `after async` already written |
| AC7 | `ac7_…` | the AC1 application | each load and Back is announced in the polite live region (`role=status`) and the focus is on the form's root in the pane (the windowed title focus is `navigation_chrome.rs`, 2026-11-01) |
| AC8 | `ac8_…` | SPA, a form that calls `SetUnsavedChanges(1)` | Back and a menu entry each ask Leave / Stay and Stay keeps the form and its address; leaving the page gets the browser's `beforeunload` question (dismissed); Leave leaves, and a navigation afterwards asks nothing |
| AC3 | `acceptance_standalone_chrome.rs` `ac3_…` | standalone, a form with `OpenFormAsync` and `OpenFormSync` buttons | `OpenFormAsync`: a new tab (`pr-form=child-form`) listed by the browser and `after async` already written; the child's `Close` ends the tab; `OpenFormSync`: a new tab, `waiting` held for 600 ms, `after sync` written when the tab closed |
| AC4 | `ac4_…` | standalone, a timer that calls `OpenFormSync`, a form event `onPopupBlocked` | headless Chrome has **no popup blocker**, so a script run before the page's own makes `window.open` answer `null`, as a blocker does: the timer's `OpenFormSync` is refused, the caller is released (`released`), *The browser blocked the new window…* is on the page, `onPopupBlocked` wrote `blocked`, and no tab of the child exists. **Firefox is not installed here: the run is Chromium only.** |
| AC5, first half | `ac5_…` | standalone, a main form (`Title` "Opener Title") that opens `child-form`; the child's buttons read `super::Title`, call `super::GetFormState()` and write `super::Title` | in the child's tab: the read waits and shows `Opener Title`; after the opener sets its own `Title` the next read shows `Retitled by the opener`; `INVOKE super::GetFormState() RETURNING` answers `Normal`; the child's write reaches the opener (`Written by the child`); neither page threw. **This run found the defect fixed in 2.0.280** (the page's glue returned an empty answer for "on its way"): before the fix the same click wrote an empty caption at once. |
| AC5, second half | `ac5_a_guarded_…` | the same application; the child has one more button whose handler is `INVOKE super::GetFormState() RETURNING WS-S ON EXCEPTION MOVE "gone" TO LBL-OUT::Caption NOT ON EXCEPTION MOVE WS-S TO LBL-OUT::Caption END-INVOKE` | with the opener open the click shows `Normal` (`NOT ON EXCEPTION`); the opener's tab is closed (the page's `pagehide` tells the child over the pair's channel); the next click shows `gone` (`ON EXCEPTION`) after about 0.2 s; the child shows **0** run-time error banners, threw nothing and wrote **no** `error` line to its console. |
| AC11 | `acceptance_spa_chrome.rs` `ac11_…` | SPA, a SideMenu entry loads `layered-form`: a base button, a layer `LYR-POP` with two labels (one inside the form, one designed to run past its right edge) and a button whose handler is `MOVE 1 TO LYR-POP::Visible` | the route (`#/layered-form`) shows the form with the layer `hidden`, `inert`, `display: none` and its label taking no room; after the button the layer is shown, its label lies inside the form's area and to the right of the menu, the root clips (`overflow: hidden`) and the pointer past the form's right edge reaches nothing of the layer |
| Two applications on one page | `two_applications_on_one_page_…` | the AC1 application (Window embed: it owns the address) plus two more mounted by `PowerRustCOBOL.mount` into boxes in the page's corners | one mounted with `{address: 'none'}` shows its own main form; its Customers entry loads `customers-form` in it and the page's address stays `#/orders-form`, where the first application's Orders entry put it; a third that asks for `{address: 'own'}` is refused, its `pr-error` detail saying *another application on this page already follows the address: mount this one with `address: "none"`*, and the other two go on. **This run found two defects, fixed in 2.0.282:** the page API dropped `address` (the loader read it, nothing passed it on, so a page could not say which application follows the address), and a second application in the threaded player failed to load with *called `Result::unwrap_throw()` on an `Err` value* — the Workers' pool (rayon's global pool) was built once per application and the instance is shared, so the second build failed; the loader now builds it once per player instance and a second application waits for it. |

**AC5's second half, how it became runnable (2.0.281, ABI A25).** Before it, a closed opener ended the child's next `super::` statement in a run-time error (`-6` "the window that opened this one has been closed", then `1104`), and nothing a `super::` call can be written in carried an `ON EXCEPTION` clause (`INVOKE` had none in the AST, the generator has no `TRY`). The operator's decision (Q3) is that a closed opener is a normal `ON EXCEPTION`, so `INVOKE super::Method(…) [RETURNING x] ON EXCEPTION … [NOT ON EXCEPTION …] [END-INVOKE]` now has that meaning in the parser (`END-INVOKE` is a reserved word), the generated module (two soft query opcodes, `0x15`/`0x16`, and the run-time's `rt_query_failed()`), the player's host (a soft query reports nothing when it fails) and the desktop interpreter. The clause is accepted in a WebAssembly module only on `INVOKE super::…`; a plain `super::` read in a `MOVE` or an `IF` cannot carry it and still ends the handler with the error — the rule is written in the System KB and in `specs/WASM-ABI-v1.md` (A25). The Developer's Guide is not touched on 2.x.x; it takes the clause when this line merges into main.

Not run here, owed: AC6's sign-in half (WASM-009 R11), AC9 (`stack: true`, WASM-012), AC4 in Firefox.

## 7. Steering compliance

- [x] i18n: the player's texts in six languages (§1.12, `src/texts.rs`); no new IDE choice was added (T22 not started).
- [x] Generated-code contract: routes and entry pages are generated from the form list by the bundle, never hand-edited; nothing here edits generated COBOL.
- [x] System KB: the web behaviour of `OpenFormSync`/`OpenFormAsync`, `super`, `SetUnsavedChanges` and the two events in the `cobolt-compiler` documentation tables, `assets/knowledge/chunked.data` rebuilt, the freshness test green.
- [x] Interpreter/binary parity: `SetUnsavedChanges` accepted by the interpreter and by the compiled binary's host.
- [x] Self-resizing windows: a standalone window gets no size from the player; the application never resizes its own window.
- [x] PRIME DIRECTIVE: no new non-Rust source; `loader.js` (tracked) only forwards configuration; browser checks are run by tooling outside the tree.
- [x] `README.md` and `docs/` untouched (2.x.x).
- [x] User code is sacred: nothing rewrites a developer's form or COBOL.
- [x] Fix vs feature: **feature** on `features-2x`; one `z` bump (2.0.127) with one `CHANGELOG.md` entry (no minor bump).
- [x] `WASM-ABI-v1.md` updated with the code: row **A20**, §12.1 (`SetUnsavedChanges`), §12.2 (the two events). A10 (a manifest field) was not needed; A11 (the suspended `super::` query) waits with T17.

## 8. Open questions (for the operator; the plan's working assumptions are in §1)

- **PQ1 (from Q4):** how a standalone project chooses new-tab or same-window navigation. Working assumption: a project sub-option `standalone_window = "new" | "same"`, default `"new"`. Alternative: a per-open choice (an extra argument of `OpenFormAsync`).
- **PQ2 (from Q4):** `OpenFormSync` in same-window navigation, where the caller's page is gone. Working assumption: a Build error naming the statement. Alternative: behave as `OpenFormAsync`.
- **PQ3 (R6, R10):** the events' names and where they are raised. Working assumption: `onRouteNotFound` and `onPopupBlocked` on the main form (the opener's form for `onPopupBlocked`).
- **PQ4 (R8):** how a form marks unsaved changes. Working assumption: the form verb `me::SetUnsavedChanges(1|0)`, a no-op on the desktop.
- Spec items still open elsewhere: which forms require sign-in (WASM-009 Q4); the mobile shell (WASM-012).

### 8.1 Decisions of the operator, 2026-11-02

- **PQ1 — per-open choice, not a project option.** The reserved route parameter `pr-window=same|new` after the form's name chooses, for that one opening, a window of its own (`new`, the default) or the page itself (`same`). There is **no** `[web] standalone_window` option and no Settings control, so §1.8's first bullet is superseded: nothing is added to `cobolt.toml`, `project_kind.rs`, `web_settings.rs` or `i18n.rs`. The parameter rides on the existing `name=value` list (`host.rs` `take_window_choice`), so no ABI field changes (A21).
- **PQ2 — `OpenFormSync` in the same window behaves as `OpenFormAsync`.** The caller's page is gone, so it waits for nobody; the Build does **not** report an error (the §1.8 working assumption is replaced).
- **PQ3, PQ4 — confirmed as built** (`onRouteNotFound`, `onPopupBlocked`, `me::SetUnsavedChanges`).
