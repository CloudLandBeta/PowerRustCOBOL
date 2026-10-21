<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Navigation and application styles (WASM-011)

- **Status:** in progress — see plan.md for what is done and measured
- **Plan:** ./plan.md   **Date:** 2026-10-20

- [x] **T1 — The two styles and the opening verbs** (R2): `host.rs::form_verb` (`OpenFormSync`, `OpenFormAsync`, `Close`), `Effect::OpenWindow` in standalone, embedded / panel / modal in the SPA.
- [x] **T2 — Routes in the address** (R4, R5): `view::shell::Route`, `sync_address`, `follow_address`, `watch_address`. Verify (Chrome 152, shell bundle): a rail click pushes `#/child-form`; Back closes the child and leaves an empty hash; Forward re-opens it; a route with other parameters re-opens the instance.
- [x] **T3 — Route parameters exposed to the program** (R4, Q1): `OpenFormSync/Async(form, "name=value", ...)`, `GetParam`, `GetParams`; address `?k=v`, standalone `pr-param-*`. Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib params_tests` (the grammar, the limits, at `onLoad`, an address's bad parameters left out).
- [x] **T4 — Standalone windows** (R9, R10 in part): `pr-form`, `pr-opener`, `pr-param-*`; a refused popup shows the notice and releases the caller with a failure; a closed window releases an `OpenFormSync` caller (polled every 500 ms).
- [x] **T5 — `super::` across windows** (R11): `src/superlink.rs`, `Host::{attach_remote_opener, remote_opener_state, apply_super_set, apply_super_call}`, `loader.js` (`pr-super`, `attach_opener`). Verify: `... --lib super_tests` (6 tests) and the two-tab Chrome run in plan.md section 6.
- [x] **T6 — Modal dialogs** (R14): `<dialog>` + `showModal()`; focus trap, Escape, stacking by the browser.
- [ ] **T7 — `super::` waits for the opener's answer** (R11, "the caller is suspended meanwhile"): **not done**; the replica answers reads at once and `CALL` is forwarded without a reply. Design: a reply message correlated by id, the task suspended like a remote call with a timeout, and `ON EXCEPTION` on timeout or close.
- [ ] **T8 — Not-found and sign-in outcomes, and the event** (R6): an address that names no form is currently ignored; the defined outcome and the developer's event are not built (the sign-in outcome needs WASM-009 R11).
- [ ] **T9 — Focus to the new form's title, the announcement, the document title** (R7): not done; a live region and focus management on each navigation.
- [ ] **T10 — Unsaved-changes guard** (R8): not done (`beforeunload`, Back, menus, with a form-level "has unsaved changes" mark).
- [ ] **T11 — Lifecycle events of a closed standalone window** (R12): the suspended caller is released; running the closed window's `onClose` as far as the browser allows (`pagehide`) is not done.
- [ ] **T12 — The popup-blocked event** (R10): the notice and the release exist; the developer-handled event does not.
- [ ] **T13 — Mobile stack** (R13): WASM-012.
- [ ] **T14 — The project setting, New Project choice, `FormFormat` build check** (R1, R3): IDE and generator.
- [ ] **T15 — Same-origin deep link of a standalone page in a clean profile, popup-blocker run** (AC): not run here.

## Done criteria

R2, R4, R5, R9, R11 (replica form), R14 are met in the player with the tests and Chrome runs named. R6, R7, R8, R12 (lifecycle), R13 and the IDE items are not met.
