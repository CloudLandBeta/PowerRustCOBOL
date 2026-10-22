<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Navigation and application styles (WASM-011)

- **Status:** in progress. T1–T7 are done and stay checked with their evidence. T8–T26 are this revision's work, against the plan
  of 2026-10-22 (operator decisions Q1–Q5, Q7 of 2026-10-22 and Q6 of 2026-10-20; R16 / AC11 added with the layers work).
- **Plan:** ./plan.md   **Date:** 2026-10-22 (first version 2026-10-20)

`[x]` was done and verified; `[ ]` is still to do. The open tasks of the 2026-10-20 list (old T7–T15) are replaced by T8–T26; the
mapping is at the end.

Paths under `src/`, `tests/`, `examples/` without a crate are `crates/cobolt-web-player/…`, its own Cargo workspace (excluded from the
main one, root `Cargo.toml` `exclude = ["crates/cobolt-web-player"]`). `PLAYER` below stands for
`--manifest-path crates/cobolt-web-player/Cargo.toml`.

**Rules for every open task.**
- **Player freshness.** A task that changes the player's sources or `crates/cobolt-wasm-abi/src` (both are `fingerprint::INPUTS`,
  `src/fingerprint.rs`) ends with `cargo run PLAYER --example build_player -- --out assets/web-player` and
  `cargo test PLAYER --test freshness` green, so the tree stays green after every task.
- **Which browser half.** The plan names `src/dom/{ops,instance,mod}.rs`. WASM-016 T22–T24 move the browser half to a Leptos
  `Surface` and retire `dom/`'s instance code. A task here lands in whichever half is current when it runs; the rule it adds lives in
  the platform-neutral part (`src/host.rs`, `src/view/shell.rs`, `src/superlink.rs`, `src/texts.rs`) wherever possible, so the move
  carries it unchanged. Likewise `crates/cobolt-web-bundle/assets/loader.js` only forwards configuration and is replaced by WASM-002's
  Rust-emitted loader (WASM-016 T7 note); a forwarding change goes into whichever loader exists.
- **ABI document.** A task that changes the ABI updates `specs/WASM-ABI-v1.md` (A10, A11, §12.1, §12.2) in the same change.
- **Operator gates.** PQ1–PQ4 (plan §8) are **not answered**. A task built on a working assumption says so; T22 is not started
  until PQ1 and PQ2 are answered. The names `onRouteNotFound`, `onPopupBlocked` (PQ3) and `me::SetUnsavedChanges` (PQ4) are working
  assumptions, renamed in one place each if the operator chooses otherwise.
- **Not touched on 2.x.x:** `README.md` and `docs/` — there is no documentation task. What 2.x.x says about itself goes in `specs/`.

## Done (plan of 2026-10-20, and R1 found built in the revision)

- [x] **T1 — The two styles and the opening verbs** (R2): `host.rs::form_verb` (`OpenFormSync`, `OpenFormAsync`, `Close`), `Effect::OpenWindow` in standalone, embedded / panel / modal in the SPA.
- [x] **T2 — Routes in the address** (R4, R5): `view::shell::Route`, `sync_address`, `follow_address`, `watch_address`. Verify (Chrome 152, shell bundle): a rail click pushes `#/child-form`; Back closes the child and leaves an empty hash; Forward re-opens it; a route with other parameters re-opens the instance.
- [x] **T3 — Route parameters exposed to the program** (R4, Q1): `OpenFormSync/Async(form, "name=value", ...)`, `GetParam`, `GetParams`; address `?k=v`, standalone `pr-param-*`. Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib params_tests` (the grammar, the limits, at `onLoad`, an address's bad parameters left out).
- [x] **T4 — Standalone windows** (R9, R10 in part): `pr-form`, `pr-opener`, `pr-param-*`; a refused popup shows the notice and releases the caller with a failure; a closed window releases an `OpenFormSync` caller (polled every 500 ms).
- [x] **T5 — `super::` across windows** (R11): `src/superlink.rs`, `Host::{attach_remote_opener, remote_opener_state, apply_super_set, apply_super_call}`, `loader.js` (`pr-super`, `attach_opener`). Verify: `... --lib super_tests` (6 tests) and the two-tab Chrome run in plan.md section 6.
- [x] **T6 — Modal dialogs** (R14): `<dialog>` + `showModal()`; focus trap, Escape, stacking by the browser.
- [x] **T7 — The style is a project setting** (R1; plan §1.1): `crates/cobolt-compiler/src/project_kind.rs` `ApplicationStyle { Spa (default), Standalone }` as `WebSettings.style` / `MobileSettings.style`; the New Project choice in `crates/cobolt-project-tools/src/create.rs` (`ApplicationStyle::parse(resolved.choice("style")).unwrap_or_default()`); changeable before a Build in `crates/cobolt-ide/src/panels/web_settings.rs` (`style_text`, Tr keys `pk_c_spa` / `pk_c_standalone` in six languages of `crates/cobolt-ide/src/i18n.rs`, `every_project_kind_string_is_in_six_languages`). The Build side (style → bundle) is T10.

## To do (plan of 2026-10-22), in execution order

- [ ] **T8 — The player's own texts in six languages** (i18n steering; R6, R8, R10 texts; plan §1.12)
  - Files: new `src/texts.rs`; `src/api.rs` (`PlayerConfig.texts`, keyed by `PlayerConfig.locale`); `src/dom/ops.rs` (`open_window`'s popup notice, `open_named`'s "The application has no form named …"); `crates/cobolt-web-bundle/src/lib.rs` (player keys in `BundleOptions.texts`); the loader forwards `texts`.
  - Do: a table for en, es, pt-BR, fr, ja, zh with keys `popup_blocked`, `route_not_found`, `leave_unsaved`, `leave`, `stay`; `config.texts` overrides a key; the two English literals go through it.
  - Verify: `cargo test PLAYER --lib texts` — every key present and non-empty in all six languages (a missing one is reported by language and key); an override wins; `cargo test -p cobolt-web-bundle` green. Player rebuilt, freshness green.

- [ ] **T9 — `FormFormat` on the web: ABI A10 and the run-time refusal** (R3, R15; plan §1.2; AC10)
  - Files: `crates/cobolt-wasm-abi/src/manifest.rs` (`FormEntry.format`, `serde(default)` = `"both"`); `crates/cobolt-web-bundle/src/lib.rs` (writes it from each `.cfrm`'s form format); `src/host.rs` (`form_verb` refuses an `Embedded` target with `CmdError::Bad`); `src/dom/ops.rs` (`resolve_menu_item`'s embed path refuses a `Standalone` one); new `src/host/nav_tests.rs`; `crates/cobolt-semantic` test; `specs/WASM-ABI-v1.md` A10.
  - Do: the build-time rule stays the one in `crates/cobolt-semantic/src/resolver.rs` `check_open_form_target` (nothing copied); the player adds only the dynamic-target refusal.
  - Verify: `cargo test PLAYER --lib nav_tests` — `Embedded` target refused, a manifest without `format` refuses nothing, a `Both` form by `OpenFormSync` in an SPA opens `OpenMode::Modal` (R3 second sentence); `cargo test -p cobolt-wasm-abi` (old manifest still parses); `cargo test -p cobolt-semantic` — a Web project's `OpenFormSync("EMBEDDED-FORM")` gives the same 049 R17 error as the desktop; `cargo test -p cobolt-semantic -p cobolt-compiler -p cobolt-form-host` green (desktop unchanged, AC10). Player rebuilt, freshness green.

- [ ] **T10 — The style reaches the bundle** (R1, R2; plan §1.1 "open end")
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (a helper `ApplicationStyle` → `AppStyle`), `crates/cobolt-web-bundle/examples/fixture_bundle.rs` (uses it).
  - Do: one mapping both the fixture and WASM-002's future `WebBuildBackend` call; the backend itself stays WASM-002's (`crates/cobolt-project-tools/src/web/mod.rs` is still `NotYetLinked`).
  - Verify: `cargo test -p cobolt-web-bundle` — the mapping is pinned for both styles and the default is SPA.

- [ ] **T11 — An SPA without a shell follows its routes; the data region is released only on destroy** (R4, R5; plan §1.4 "Gap"; AC1)
  - Files: `src/dom/ops.rs` (`follow_address`: today it returns early when `!shell_on`), `src/dom/instance.rs` (`sync_address`).
  - Do: without a SideMenu, a followed route opens the named form as `OpenMode::Panel` over the main form, so every route the player pushed can be followed back.
  - Verify: `cargo test PLAYER --lib` green; the AC1 browser run of T25 also asserts with `pr_dbg_live` that a resident or `PreservePreviousForm` form keeps its region and a destroyed one releases it. Player rebuilt, freshness green.

- [ ] **T12 — Not-found outcome and its event** (R6 first half; plan §1.5; AC6 first half) — *event name is PQ3's working assumption*
  - Files: `src/dom/ops.rs` (`follow_address`, `open_named`), `src/host.rs` (raise the event), `src/host/nav_tests.rs`, `specs/WASM-ABI-v1.md` §12.2.
  - Do: the main form stays shown, the `route_not_found` notice (T8) is shown, the route is **replaced** (`replaceState`), and `onRouteNotFound` is raised on the main form with the route text as its argument.
  - Verify: `cargo test PLAYER --lib nav_tests` — the event is raised with the route text; browser half in T25 (AC6: notice, event, Back does not return to the bad address).

- [ ] **T13 — The sign-in hook for a protected route** (R6 second half; plan §1.5; AC6 second half) — **blocked on WASM-009 R11 / Q4**
  - Files: `src/dom/ops.rs` (`follow_address` hands the route to the Authentication gate; the gate re-follows it after sign-in).
  - Verify: when WASM-009's gate exists, the AC6 browser run: a protected form's address without a session shows sign-in, then the requested form. Until then this task stays open and the plan's status says so.

- [ ] **T14 — The popup-blocked event** (R10; plan §1.7; AC4) — *event name is PQ3's working assumption*
  - Files: `src/dom/ops.rs` (`open_window`'s refusal branch), `src/host.rs`, `src/host/nav_tests.rs`, `specs/WASM-ABI-v1.md` §12.2.
  - Do: on a refused `window.open`, besides the notice (now T8's text) and `Inbox::ChildClosed` (built), raise `onPopupBlocked` on the opener's form with the form name.
  - Verify: `cargo test PLAYER --lib nav_tests` — event raised and the `Wait::Child` released; AC4 in T25 (Chromium and Firefox, popups blocked, timer-driven open).

- [ ] **T15 — Focus, announcement, scroll on every navigation** (R7; plan §1.6; AC7)
  - Files: `src/dom/mod.rs` (one polite `aria-live` region beside the existing hosts), `src/dom/instance.rs` (on `Effect::Opened`), `src/view/*` (the title made focusable, `tabindex="-1"`).
  - Do: for `Embedded`, `Root`, `Panel` and a standalone start form: focus the title, write the form's own title into the live region, scroll the content to the top; a modal focuses its own title. No new strings (the title is the developer's).
  - Verify: `cargo test PLAYER --lib` (the view emits the focusable title and the live region); `cargo test PLAYER --test sinks` green; AC7 in T25 (an accessibility check by browser tooling outside the tree).

- [ ] **T16 — Unsaved-changes guard** (R8, R15; plan §1.6; AC8) — *verb name is PQ4's working assumption*
  - Files: `src/host.rs` (`form_verb`: `SetUnsavedChanges` sets a per-instance mark), `src/dom/ops.rs` (`beforeunload` while any instance is marked; Back re-pushes the route it leaves and asks the in-page confirm with T8's `leave_unsaved` / `leave` / `stay`; a menu load asks the same), `crates/cobolt-runtime/src/interpreter.rs` (the form-verb dispatch beside `"OPENFORMSYNC" | "OPENFORMASYNC"`: accept it as a no-op; `is_known_method` if the inline form must reach it), the compiled binary's host (parity: find where it dispatches form verbs and accept it there too), `crates/cobolt-semantic/src/resolver.rs` (the verb is not reported unknown), `specs/WASM-ABI-v1.md` §12.1.
  - Verify: `cargo test PLAYER --lib nav_tests` (the mark set and cleared); `cargo test -p cobolt-runtime` — a desktop interpreter test that the verb is a no-op; the same program built as a binary runs it without error (interpreter/binary parity); `cargo test -p cobolt-semantic` green; AC8 in T25 (Back and tab close prompt).

- [ ] **T17 — `super::` suspends the caller until the opener answers** (R11; Q3 decided; plan §1.7; AC5)
  - Files: `src/superlink.rs` (requests `{"type":"get"|"set"|"call","id":N,…}`, replies `{"id":N,"value":…}` / `{"id":N,"error":…}`; an older peer without ids is answered from the replica), `src/host.rs` (`Wait::Super { id, done }` beside `Wait::Child` in `enum Wait`, timeout `PlayerConfig.call_timeout_ms`, `closed` answers the error), `src/host/super_tests.rs`, `crates/cobolt-wasm` (WASM-010 phase C: a ctrl-`1` `GET`/`CALLR` answered "suspended, re-issue after resume" returns `SUSPENDED` and re-issues on `pr_resume`), `specs/WASM-ABI-v1.md` A11.
  - Do: the replica stays as the data a re-issued query reads, not as the answer. Q3: one origin and one profile only; a closed opener is a normal `ON EXCEPTION`.
  - Verify: `cargo test PLAYER --lib super_tests` — `Wait::Super` resumed by a reply, by a timeout and by `closed`, the last two ending in the `ON EXCEPTION` path; `cargo test -p cobolt-wasm` (the re-issue on resume); AC5 in T25 with a fixture **that has** `ON EXCEPTION`. Player rebuilt, freshness green.

- [ ] **T18 — Lifecycle of a closed standalone window** (R12; plan §1.7)
  - Files: `src/superlink.rs` / `src/api.rs` (`pagehide`: the child runs its closing handlers through `pr_close_form`; a handler that suspends there is abandoned with `pr_cancel`), `src/host.rs` (the opener's events for that handle when `Inbox::ChildClosed` arrives).
  - Verify: `cargo test PLAYER --lib` — a host test that `ChildClosed` raises the opener-side events and releases `Wait::Child` (built); the AC3 browser run of T25 records what Chromium runs on `pagehide` and the plan names what is not guaranteed.

- [ ] **T19 — Several applications on one page: one address owner** (R5, Q7 decided; plan §1.9)
  - Files: `src/api.rs` (`PlayerConfig.address`; mount refuses a second owner with a message; `PANIC_FN` keyed by application if needed), `src/dom/instance.rs` (`sync_address` only for the owner), `src/dom/ops.rs` (`follow_address`, `watch_address` only for the owner), `src/dom/apply.rs` (`FOCUS`), `crates/cobolt-web-bundle/src/lib.rs` (`options.embed.address`).
  - Do: an `Embed::Window` application owns the hash; an `Embed::Element` one does not touch the address unless `address: "own"`; a non-owner keeps its chain in memory. First establish (and record here) whether two loaders on one page share one player wasm instance; key the panic hook by application only if they do.
  - Verify: `cargo test PLAYER --lib`; `cargo test -p cobolt-web-bundle` (`embed.address` written); T25's two-applications run: only the owner writes the hash. Player rebuilt, freshness green.

- [ ] **T20 — Mobile: the SPA's chain as a stack** (R13, Q5 decided; plan §1.10; AC9 in part)
  - Files: `src/api.rs` (`PlayerConfig.stack: bool`), `src/view/shell.rs` / `src/dom/instance.rs` (`Panel` / `Modal` full-screen when `stack`), the loader forwards `stack`.
  - Do: SideMenu load, `OpenFormAsync` and `OpenFormSync` push a screen; Back pops it through the existing `popstate` path; `OpenFormSync`'s caller is released when its screen is popped. The mobile shell and the OS deep link are WASM-012's.
  - Verify: `cargo test PLAYER --lib` (full-screen rendering with `stack`); AC9 in T25 with `stack: true` in a browser; the WASM-012 shell run stays open until that shell exists. Player rebuilt, freshness green.

- [ ] **T21 — Layers inside the instance's root** (R16; plan §1.11; AC11) — **blocked on ABI A9 in `FormEntry` (WASM-004 / WASM-016 T21)**
  - Files: `src/dom/instance.rs` (or the Leptos surface): layers drawn inside `section.pr-form`, clipped to the form's area, never in the page-level `pr-layer` host of `src/dom/mod.rs`.
  - Do: a route opens a fresh instance with every layer hidden; a route to the shown instance with the same parameters leaves its layers as they are; `Route` carries no layer state.
  - Verify: AC11 in T25 once `cobolt-wasm-abi::manifest::FormEntry` carries `layers`.

- [ ] **T22 — Same-window navigation in the standalone style** (Q4 decided; plan §1.8) — **gated on PQ1 and PQ2; do not start before the operator answers**
  - Files (working assumption): `crates/cobolt-compiler/src/project_kind.rs` (`[web] standalone_window = "new" | "same"`, `serde(default)` = `"new"`), `crates/cobolt-ide/src/panels/web_settings.rs` + `crates/cobolt-ide/src/i18n.rs` (the choice, shown only for Standalone, Tr keys ×6), `src/dom/ops.rs` (`OpenFormAsync` → `location.assign`, no `pr-super`), `crates/cobolt-semantic/src/resolver.rs` (PQ2 assumption: `OpenFormSync` is a Build error naming the statement).
  - Verify: `cargo test -p cobolt-compiler` (old `cobolt.toml` loads as `"new"`); `cargo test -p cobolt-ide --bin cobolt-ide i18n` (new keys in six languages) and the settings frame drawn in each language; `cargo test -p cobolt-semantic`; `cargo test PLAYER --lib`; a browser run: Back reloads the opener from its address and parameters, `super::` fails with `ON EXCEPTION`.

- [ ] **T23 — System KB** (steering: System KB rule; spec §6)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the documentation section on `OpenFormSync` / `OpenFormAsync`, `FormFormat` and `super`: the web behaviour per style, the run-time `FormFormat` refusal, `super::` across windows and its `ON EXCEPTION`, `SetUnsavedChanges`, `onRouteNotFound`, `onPopupBlocked`, Q1's "form and parameters, never data"), `assets/knowledge/chunked.data`.
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb`; `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green.

- [ ] **T24 — Desktop unchanged** (R15; AC10 second half)
  - Verify: `cargo test --no-fail-fast -p cobolt-semantic -p cobolt-compiler -p cobolt-form-host -p cobolt-runtime` green; every `test result` line read.

- [ ] **T25 — Browser acceptance runs** (AC1–AC9, AC11; plan §6) — run by tooling outside the tree (PRIME DIRECTIVE)
  - Do: through `crates/cobolt-web-bundle/examples/fixture_bundle.rs` bundles in headless Chrome (and Firefox for AC4): AC1 (three SideMenu forms, address, Back, reload, `pr_dbg_live`); AC2 (statement after `OpenFormSync` not run until close; `OpenFormAsync` runs on); AC3 (a new tab seen by the browser; caller resumed at close; T18's `pagehide` record); AC4; AC5; AC6 first half (second half with T13); AC7; AC8; AC9 with `stack: true`; AC11 when T21 is unblocked; two applications on one page (T19).
  - Verify: each run is recorded in plan.md §6 with the browser version, the bundle and what was seen; the spec's AC boxes are checked only for runs that passed. AC6 second half, AC9 in the mobile shell and AC11 stay open, named, while their dependencies (WASM-009 R11, WASM-012, ABI A9) are missing.

- [ ] **T26 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs` (`VERSION`: bump `z` once, from the value current at that time), `CHANGELOG.md` (one entry above the previous heading, dated, *Written on `features-2x`*, naming what landed and what stays open: T13, T21, T22, PQ1–PQ4).
  - Verify: `cargo test PLAYER --no-fail-fast` (lib, `abi_host`, `sinks`, `dependency_guard`, `freshness`); `cargo test --no-fail-fast -p cobolt-wasm-abi -p cobolt-web-bundle -p cobolt-wasm -p cobolt-semantic -p cobolt-runtime -p cobolt-compiler -p cobolt-form-host`; `cargo test --no-fail-fast -p cobolt-ide --bin cobolt-ide` (and `-p cobolt-project-tools` if T22 ran) — every `test result` line read; no warnings, no `allow`; `README.md` and `docs/` untouched (`git status`).

## Old open tasks → this list

| 2026-10-20 | Now |
|---|---|
| T7 `super::` waits | T17 |
| T8 not-found / sign-in | T12, T13 |
| T9 focus / announcement | T15 |
| T10 unsaved changes | T16 |
| T11 closed-window lifecycle | T18 |
| T12 popup-blocked event | T14 |
| T13 mobile stack | T20 |
| T14 project setting / New Project / `FormFormat` | T7 (done), T9, T10 |
| T15 deep link / popup-blocker run | T25 |

## Done criteria

Every acceptance criterion of spec.md is checked by a passing task, or named as waiting for its dependency (AC6 second half: WASM-009 R11;
AC9 in the shell: WASM-012; AC11: ABI A9). Tests pass with `--no-fail-fast`; the System KB and `chunked.data` are current; the player is
rebuilt and its freshness test green; PQ1–PQ4 answered or still marked as working assumptions. No commit or push from this phase.
