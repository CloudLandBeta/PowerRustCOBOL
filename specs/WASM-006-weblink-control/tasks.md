<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Hyperlink control (WASM-006)

- **Status:** in progress. T1–T4 (the player half, written against the interim `WebLink` name) are done; the rest follows the plan of 2026-10-22.
- **Plan:** ./plan.md   **Date:** 2026-10-22
- **Branch:** `features-2x` (2.x.x). Feature. **`README.md` and `docs/` are never touched**; what 2.x.x says about the control lives in specs/ and the System KB.

Ordered so the tree stays green after each task. Every claim about existing code below was read on 2026-10-22 (path and symbol given). Test sweeps run with `--no-fail-fast` and every `test result` line is read; `cobolt-forms` needs `--features render`, `cobolt-ide` needs `--bin cobolt-ide`.

**Open questions carried, with the working assumption (none decided here):**

- **OQ1 — `onNavigated` when the page unloads** (plan §7): assumed raised for fragment links, router navigations and new tabs/windows only. *To confirm.*
- **OQ2 — Safari's modifier table** (plan D3): verified against Apple's *Safari keyboard and other shortcuts* page at the start of T5, before it is coded; the rows are recorded in plan.md with the page's date. **Done 2026-10-23** (plan D3): ⌘⌥-click → new window was not on the page and is dropped; ⇧⌘-click → new active tab added.
- **OQ3 — the project setting.** The *carrier* is settled here from the code (T11): it follows `[agents] file_memory_limit_mb` exactly — a reader in `cobolt-compiler` over `CoboltProject` (`cobolt-compiler/src/lib.rs:1008`, like `project_file_memory_limit` `:1071`), published to a process-global by `rcrun run-form` (`cobolt-cli/src/form_gui.rs:228-245`) and baked into a built application's generated main (`cobolt-compiler/src/lib.rs:4344-4349`), read by every form the process hosts through `cobolt-form-host/src/seeding.rs` (the `OnceLock` pattern of `publish_connections` `:51`). The *name* `[links] schemes` stays the plan's proposal. *To confirm.*
- **OQ4 — the Windows opener** (plan D7): `rundll32 url.dll,FileProtocolHandler` assumed; confirmed on a Windows machine in T14 or T19.
- **Spec §6 Safety** — R7/R8 reviewed against WASM-013 in `/analyze`, not here.

---

## Done (player half, interim name `WebLink`)

These boxes stay checked with their evidence. T5 and T8 move and rename this code; its tests move with it and must stay green.

- [x] **T1 — The link policy** (R7, R8): `crates/cobolt-web-player/src/weblink.rs::{normalize, scheme_of, kind_of, check}`; extra project schemes through `Policy`; the dangerous schemes never allowed.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib weblink` green (7 tests).
- [x] **T2 — The click plan and the properties** (R3, R5, R8): `plan`, `LinkProps::read`; Alt-click download; `noopener noreferrer`; `ping` never written.
- [x] **T3 — Rendering as an anchor** (R2, R4, R9): `view/controls.rs::weblink`, `view/interact.rs::weblink`. Chrome 152 run recorded in the plan of 2026-10-20 (anchor, `rel`, no `ping`, `javascript:` refused by `SetUrl`, `mailto:` accepted, plain click not prevented); evidence for the old code, re-run in T18 after the move.
- [x] **T4 — Methods, player dispatch** (R12): `weblink.rs::method` — `GETURL`, `SETURL` (refusal through `check`), `OPEN` and `OPENIN` queue the page commands `open` / `open-in`.
  - **Gap found 2026-10-22 (plan C9):** `dom/instance.rs::run_command` (`:468`) has no arm for `open` / `open-in` and drops them at `_ => {}` (`:494`), so `Open` and `OpenIn` do nothing in a browser yet. Closed by T9; this box covers the dispatch only.

---

## To do

- [ ] **T5 — One link policy in `cobolt-forms`** (R5, R7, R8; Q2, Q6; plan C3, C5, C6, D3, D4, D9)
  - Files: root `Cargo.toml` (`url` into `[workspace.dependencies]`; it is already in `Cargo.lock` at 2.5.8), `crates/cobolt-forms/Cargo.toml` (`url.workspace = true`), `crates/cobolt-forms/src/hyperlink.rs` (**new**), `crates/cobolt-forms/src/lib.rs` (module), `crates/cobolt-web-player/src/weblink.rs` (keeps only `LinkProps::read` and `method`, re-using the moved items).
  - Do: first verify OQ2 (Safari table) and write it into plan.md D3. Move `UrlKind`, `normalize`, `scheme_of`, `kind_of`, `Refused`, `Policy`, `check`, `Target`, `Click`, `NavPlan`, `plan` from `weblink.rs` into `cobolt_forms::hyperlink`, pure and `wasm32`-clean (no egui, no I/O). Add the per-OS modifier table (`Platform::{Apple, Other}`; Chrome rows for Linux/Windows, Safari rows for macOS/iOS/iPadOS) and the parse step (`url::Url::parse` / `Url::join` against a base; percent-encoding and IDN are the parser's; the parsed form is what `check` sees). Move the seven tests with the code.
  - Verify: `cargo test -p cobolt-forms --features render hyperlink -- --nocapture` green — the seven moved tests plus new ones: the table for macOS, Linux and Windows (Shift-click on Apple = plain click; ⇧⌘-click = new active tab; ⌘⌥-click = new tab, not a new window); percent-encoding, an IDN host shown in its parsed form, a relative address against a base; `JaVaScRiPt:` / `\tjavascript:` / `vbscript:` refused after parsing too. `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib` green (player builds against the moved policy). `cargo check --target wasm32-unknown-unknown -p cobolt-forms` (or the player's wasm build) proves the module stays wasm-clean.

- [ ] **T6 — `ControlType::Hyperlink` in the model** (R1, R2, R3, R13; Q1; plan C1, C7)
  - Files: `crates/cobolt-forms/src/model.rs` — the enum (`:2746`), `ALL` (`:2926`), `as_str`/`from_str` (`"Hyperlink"`; `from_str("WebLink")` → `Hyperlink`, saved as `Hyperlink`), `default_size` (`:3142`, the Label's 120×20), `primary_event` (`:3208`, `onClick`), `supported_events` (`:3371`: the Label's list plus `onBeforeNavigate`, `onNavigated`), `is_non_visual` (`:4152`, false), `has_perspective` (`:4181`, as the Label), `takes_tab_order` (`:7686`, true), the `Control::new` arm (the Label's seeds plus `Cursor = Hand`, `Url`, `Target = Self`, `WindowFeatures`, `Rel`, `Download`, `HrefLang`, `MimeType`, `ReferrerPolicy`, `Underline = Always`, `AccessibleLabel`, `LinkColor`, `HoverColor`, `ActiveColor`, `VisitedColor`; **no `Ping`**), `runtime_property_names_for` (`:1527`, the run-time flag `Cancel`).
  - Do: a `.cfrm` whose stored `Url` is refused is loaded as written (user data is sacred, D5); the refusal is reported by T15/T13, never by clearing it.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green — new tests: `.cfrm` round trip with every property at a distinct value (AC7); `type="WebLink"` loads as Hyperlink and saves as `Hyperlink`; `Control::new(Hyperlink)` seeds `Cursor = Hand` and no `Ping`; a stored `javascript:` Url survives load and save unchanged. Then the `ControlType::ALL`-driven suites in the crates that hold them build and pass (run in T21's sweep; a red one here is fixed here).

- [ ] **T7 — The method vocabulary and the `Cancel` property** (R6, R12; plan risk "`Cancel` collides with a method name")
  - Files: `crates/cobolt-ast/src/methods.rs::is_known_method` (`:85`; `OPEN` is at `:193`, `CANCEL` at `:152`) — add `SETURL`, `GETURL`, `OPENIN`; the parser only if the tests below fail.
  - Verify: `cargo test -p cobolt-ast -p cobolt-parser --no-fail-fast` green with new tests: the three names are methods; `SET LNK-1::Cancel TO TRUE` and `IF LNK-1::Cancel` parse as the property, not a method call; `INVOKE LNK-1 "OpenIn" USING "NewTab"` parses.

- [ ] **T8 — The player and the bundle on the model's type** (R1, R3, R4; plan C1)
  - Files: `crates/cobolt-web-player/src/` — `weblink.rs` → `hyperlink.rs`; `kinds.rs` (`PlayerKind::WebLink` removed `:135`, a `TABLE` row `Hyperlink` class A, the count tests at `:223`/`:230` become 63, the `from_str("WebLink")` tests at `:237`/`:244` rewritten for the alias); `view/controls.rs` (`weblink` → `hyperlink`; writes `hreflang`, `type`, `aria-label` from `AccessibleLabel`, `title` from `Tooltip`; never `ping`); `view/interact.rs`, `view/style.rs`, `css.rs`, `base_css.rs` (`pr-weblink` → `pr-hyperlink`, state colours `--pr-link`/`--pr-hover`/`--pr-active`/`--pr-visited`, `:visited` colour only, D8); `methods.rs` (dispatch by `ControlType::Hyperlink`); `tree.rs`, `view/viewer.rs`, `view/contract.rs`, `api.rs` (`weblink_schemes` `:63`, `:782`), `host.rs`, `lib.rs`, `dom/ops.rs` (imports). `crates/cobolt-web-bundle/src/lib.rs` (`BundleOptions::weblink_schemes` `:153`/`:186`/`:677` → `link_schemes`, reading the old key as an alias), `assets/loader.js` (`:687`, the key rename only — PRIME DIRECTIVE: no new non-Rust source), `tests/support/gallery.rs` (fixture → `Hyperlink`).
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` and `cargo test -p cobolt-web-bundle --no-fail-fast` green — new tests: `kinds` has 63 rows; the anchor carries `hreflang`, `type`, `aria-label`, `title` and no `ping`; a `config.json` with the old `weblink_schemes` key still widens the policy. `Grep weblink|WebLink` over `crates/` returns only the alias sites and their tests.

- [ ] **T9 — Player: the missing page commands and fragment links** (R10, R12; plan C9)
  - Files: `crates/cobolt-web-player/src/dom/instance.rs::run_command` (`:468`) — arms `open`, `open-in`, `scroll-to`; `dom/ops.rs::navigate` (`:131`, the parse step through `web_sys::Url::new_with_base` against `document.baseURI`); `hyperlink.rs` (a `#name` resolves to a control by name, case-insensitive, or to an element id).
  - Do: `open` acts as a plain click honouring `Target`; `open-in` takes the target argument; a new-context open without a gesture that the browser blocks reports the existing pop-up notice (`dom/ops.rs::open_window` `:154`). `scroll-to` scrolls the element into view and focuses it.
  - Verify: player tests green with a C9 regression test (each of `open`, `open-in`, `scroll-to` reaches a handled arm, not `_`); headless acceptance of the fragment link in T18 (AC5).

- [ ] **T10 — Player: Cancel and the navigate events** (R5, R6, R13; Q3; plan C4, D1, D2; OQ1)
  - Files: `crates/cobolt-web-player/src/view/interact.rs` (the click branch), `hyperlink.rs`, `tree.rs` (`Ctl::has_handler` `:125`), `specs/WASM-ABI-v1.md` (amendment **A10**: `onBeforeNavigate` with argument `target⇥url`, `onNavigated` with argument `url` in §12.2; the §12.1 `Navigate`/`OpenUrl` row cites R7, not R4).
  - Do: no handler bound to `onClick` or `onBeforeNavigate` → the anchor stays native. A bound handler → default prevented; `Cancel` reset to false; `onClick` (argument: the modifiers text) → `onBeforeNavigate` (argument: the planned `target⇥url`); a true `Cancel` after either stops it; otherwise navigate from script (`location.assign` for `Self`/`Parent`/`Top`, `window.open` with `noopener,noreferrer` unless `Rel` loosens it, an anchor `click()` for a download). `onNavigated` per OQ1's working assumption. Middle-click goes through `auxclick`.
  - Verify: player tests green — the bound branch prevents the default and the unbound one does not; Cancel set in `onClick` and in `onBeforeNavigate` each stop the plan; the modifier argument per platform. Headless AC2/AC3/AC6 in T18.

- [ ] **T11 — The project's extra link schemes** (R7 "configurable per project, never widened by default"; OQ3)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`CoboltProject` `:1008` gains an optional `[links] schemes`, empty by default; a reader `project_link_schemes(manifest)` beside `project_file_memory_limit` `:1071`; the generated main bakes it and publishes it beside `publish_file_memory_limit` at `:4349`); `crates/cobolt-form-host/src/seeding.rs` (`publish_link_schemes` / `link_schemes()`, first call wins, as `publish_connections` `:51`); `crates/cobolt-cli/src/form_gui.rs` (publish in the manifest block `:233-245`); `crates/cobolt-ide/src/project_model.rs` (`CoboltProject` `:40`, so the IDE reads and preserves the key). The web Build fills `BundleOptions::link_schemes` from the same reader where WASM-002 constructs `BundleOptions` (today only tests and `examples/fixture_bundle.rs` do).
  - Verify: `cargo test -p cobolt-compiler -p cobolt-form-host --no-fail-fast` green — a manifest without `[links]` gives an empty list; `schemes = ["javascript", "ftp"]` widens for `ftp` only; the generated main's source contains the publish call (as the existing test at `cobolt-compiler/src/lib.rs:10276` does for connections); a `cobolt.toml` round trip through the IDE keeps the key.

- [ ] **T12 — Desktop faces: Designer and run-form** (R2, R4, R9, R14; memory: controls are drawn by two paths)
  - Files: `crates/cobolt-forms/src/paint.rs` (Designer face: Caption as link text in `LinkColor`, `Underline` honoured, in-place Caption edit as the Label's), `crates/cobolt-forms/src/render.rs` (run-form face beside the Label's; hover/active colours; Enter activates; focus ring; accesskit name = `AccessibleLabel` or Caption; the click outcome carries the modifiers and the planned navigation from `cobolt_forms::hyperlink::plan`).
  - Do: the face reuses the Label's text path; if the change touches any border geometry, stroke, radius or clipping, run the GUI Border Validation Agent (`.agents/agents/gui-border-validation-agent.md`) before closing the task.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green — new tests measure the paint (memory: contrast tests must measure the paint): the caption galley is painted in `LinkColor`, an underline shape exists for `Always`, none for `Never`, only while hovered for `Hover`; the render outcome of a Ctrl-click (Linux) / ⌘-click (macOS) reports a new tab; Enter on a focused Hyperlink produces a click outcome.

- [ ] **T13 — Desktop runtime: methods, Cancel, the navigate request** (R6, R7, R12, R13, R16; AC4, AC6)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` — Hyperlink arms beside the Label's `"SETCAPTION"` (`:16387`): `SETURL` (parse + `check` with `seeding`'s published schemes; refused → control unchanged, refusal reported through `LastError` as the other refusals are), `GETURL`, `OPEN`, `OPENIN`; `Cancel` reset before `onClick`, read after `onClick` and `onBeforeNavigate`; when not cancelled, write `_NavigateRequest` (the parsed URL and target); `onNavigated` raised for a fragment link.
  - Verify: `cargo test -p cobolt-runtime --no-fail-fast` green — `SetUrl("javascript:alert(1)")`, `vbscript:`, `data:text/html,…` each refused, `GetUrl` unchanged, refusal reported (AC4); Cancel set in `onClick` → no `_NavigateRequest`, set in `onBeforeNavigate` → none, not set → exactly one (AC6); `Open` produces a request honouring `Target`.

- [ ] **T14 — Desktop host: opening the system browser** (R10, R16; AC10; plan C10, D7; OQ4; interpreter/binary parity)
  - Files: `crates/cobolt-form-host/src/os_handoff.rs` (`Handoff` `:40` gains `OpenUrl`, request prop `_NavigateRequest`; `command_for` `:84` — macOS `open`, Linux `xdg-open`, Windows `rundll32 url.dll,FileProtocolHandler`, the URL as one argument, **never** `cmd /C start`; run on a thread as `hand_off_async` `:139`); `crates/cobolt-form-host/src/host.rs` (`viewer_os_request` `:1461` and `drive_viewer_os_handoffs` `:1487` gain the `_NavigateRequest`/`_NavigateAnswer` pair; the no-handler path takes the request straight from the render outcome; a fragment link scrolls the form's scroll area and moves focus, R10; `mailto:`/`tel:`/`sms:` go to the same OS opener).
  - Do: `cobolt-form-host` is the one host of `rcrun run-form`, embedded child forms and the compiled binary, so one change reaches all three; confirm the generated main of `cobolt-compiler` routes through the same `drive_viewer_os_handoffs`.
  - Verify: `cargo test -p cobolt-form-host --no-fail-fast` green — `command_for(OpenUrl, os)` for `macos`/`linux`/`windows`, the URL `https://x.test/?a=1&b=2` passed as one argument with `&` intact and no shell program in the command; a request with a refused scheme never reaches the opener and is reported; an unavailable `xdg-open` is reported through `LastError` as the Share path does. Live AC10 is T19.

- [ ] **T15 — IDE: toolbox, Properties panel, Designer, agent, i18n ×6** (R1, R4, R14, R15)
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs` (`TOOLS` beside the Label entry `:32`, its icon; every project kind), `panels/properties.rs` (enum editors for `Target`, `Underline`, `ReferrerPolicy`; colour editors for the four state colours; the `Url` field with live parse + `check` + the parsed form under it + a refusal indication, including for a refused value loaded from `.cfrm`), `designer.rs`, `agent.rs`, `prop_help_data.rs`, `ide_text_data.rs`, `i18n.rs` — Tr keys in all six languages for the toolbox label and tooltip, every Properties label and help line, and the refusal message. No "cobolt" in user-facing text.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` green, including the i18n tests (no empty translation in any of the six languages) and a Properties-panel test that a `javascript:` Url shows the refusal and keeps the stored text. The visual check in six languages is the operator's (T19).

- [ ] **T16 — System KB** (R12, R15; AC7; steering: System KB rule)
  - Files: `crates/cobolt-compiler/src/lib.rs` — `property_reference_for` (`:6380`, every property above, `Cancel` included, `ping` stated absent), `event_reference` (`:7646`, `onBeforeNavigate`, `onNavigated`), `control_structure` (`:7797`), `control_purpose` (`:7809`), `control_method_docs` (`:7885`, `SetUrl`, `GetUrl`, `Open`, `OpenIn`, `SetCaption`, `GetCaption`), `control_usage_notes` (`:8412`: the scheme rules, the per-OS modifier table, "do not wait in a link's handler before a new tab", desktop always opens the system browser, `:visited` = colour only); `crates/cobolt-compiler/src/runtime_features.rs` only if a feature flag is needed; `assets/knowledge/chunked.data` regenerated with `cargo run -p cobolt-ide --example build_chunked_kb`. COBOL examples in English, written with the extensions (`SET LNK-1::Cancel TO TRUE`).
  - Verify: `cargo test -p cobolt-compiler --no-fail-fast` green; `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green (the freshness test lives in `crates/cobolt-ide/src/grace_host.rs`, not in the compiler as plan §6 says); `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness` green.

- [ ] **T17 — `tests/controls/hyperlink.rs`** (R15; AC7)
  - Files: `tests/controls/hyperlink.rs` (**new**), `tests/controls/Cargo.toml` (the `[[test]]` entry if the package lists them).
  - Verify: `cargo test --manifest-path tests/controls/Cargo.toml --test hyperlink -- --nocapture` green — every property set to a distinct value, saved and reloaded, printed.

- [ ] **T18 — Headless browser acceptance** (AC1, AC2, AC3, AC5, AC6, AC8)
  - Files: none in the tree beyond the gallery fixture of T8; the harness is WASM-015's, any non-Rust driver lives **outside** the repository (PRIME DIRECTIVE).
  - Verify, on the gallery bundle, in Chromium and WebKit, each run printing what it measured: AC1 (an `<a>` with text "Example", computed `cursor: pointer`); AC2 (each `Target`; Ctrl/⌘-click and middle-click → new tab; Shift-click / ⌘⌥-click → new window per platform; the click argument reports the modifiers); AC3 (`window.opener` null with empty `Rel`, as set otherwise); AC5 (`mailto:` with subject and body, `tel:`, relative, fragment → scroll + focus, `Download`); AC6 (Cancel stops, no Cancel navigates); AC8 (Tab reaches the link, Enter follows it, focus ring visible, the accessibility check reports no critical issue). A failure (e.g. Safari's middle-click prevention) is recorded in plan.md, not hidden.

- [ ] **T19 — Operator checks on desktop** (AC10; R14; OQ4) — the agent never drives the application window.
  - Verify (operator): a Hyperlink opens the system browser under `rcrun run-form`, in an embedded child form and in a compiled binary; a `javascript:` Url stays refused in all three; the Designer face and the Properties panel's refusal indication in six languages; the Windows opener confirmed on Windows (OQ4). Recorded in this file with the date.

- [ ] **T20 — Mobile shell** (R11; AC9) — **blocked on WASM-012.** The player's routing switch (shell detected from the host page's configuration; external origins and `mailto:`/`tel:`/`sms:` to the shell's opener) lands with the shell; AC9 is run on a device or simulator then. Listed as not run until WASM-012 exists.

- [ ] **T21 — Finalize**
  - Bump `z` once in `crates/cobolt-ide/src/version.rs` (`VERSION`, `2.0.29` when this was written — bump whatever is current); add one `CHANGELOG.md` entry above the previous `## [PowerRustCOBOL …]` heading, dated, with the line `*(Written on `features-2x`, after 2.0.N.)*`. No documentation task: `README.md` and `docs/` are not touched on 2.x.x.
  - Credential scan of the staged diff (no `.cfrm` example with a key).
  - Verify: `cargo fmt --check`; the tests of every crate touched, `--no-fail-fast`, every `test result` line read: `cargo test -p cobolt-forms --features render -p cobolt-ast -p cobolt-parser -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler -p cobolt-cli -p cobolt-web-bundle --no-fail-fast`; `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`; `cargo test --manifest-path tests/controls/Cargo.toml --no-fail-fast`. Check `df` before the sweep (memory: disk exhaustion masquerades as build errors).

## Acceptance criteria → tasks

| AC | Covered by |
|---|---|
| AC1 — anchor text and `cursor: pointer` | T3, T8, T18 |
| AC2 — Targets, modifiers, click argument | T5, T10, T12, T18 |
| AC3 — `noopener` with empty `Rel` | T1/T2 (kept), T5, T10, T18 |
| AC4 — dangerous schemes refused in designer and `SetUrl` | T5, T13, T15 (and T1, T4 in the player) |
| AC5 — `mailto:`, `tel:`, relative, fragment, `Download` | T5, T9, T18 |
| AC6 — Cancel | T7, T10, T13, T18 |
| AC7 — `.cfrm` round trip, `ALL` tests, KB freshness | T6, T16, T17, T21 |
| AC8 — keyboard, focus ring, accessibility | T8, T12, T18 |
| AC9 — mobile shell | T20 (blocked on WASM-012) |
| AC10 — desktop opens the system browser in three hosts | T11, T13, T14, T19 |

## Done criteria

Every AC above checked with its evidence (AC9 may stay open, blocked on WASM-012, and is said so); the tests of T21 green; OQ1–OQ4 answered or still carried with their working assumptions stated; no commit or push from this phase.
