<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — WebLink control (WASM-006)

- **Status:** in progress — the player half is done; Cancel / before-navigate, fragments and the IDE half are not
- **Plan:** ./plan.md   **Date:** 2026-10-20

- [x] **T1 — The link policy** (R7, R8): `src/weblink.rs::{normalize, scheme_of, kind_of, check}`; extra project schemes through `Policy`; the dangerous schemes never allowed.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib weblink` green (7 tests).
- [x] **T2 — The click plan and the properties** (R3, R5, R8): `plan`, `LinkProps::read`; Alt-click download; `noopener noreferrer`; `ping` never written.
- [x] **T3 — Rendering as an anchor** (R2, R4, R9): `view/controls.rs::weblink`, `view/interact.rs::weblink`. Chrome 152 run recorded in plan.md section 6.
- [x] **T4 — Methods** (R12): `SetUrl`, `GetUrl`, `Open`, `OpenIn`.
- [ ] **T5 — Cancel and the before-navigate event** (R6, R13, AC for Cancel): **not done.** The browser navigates after the click and a handler cannot stop it. Plan in plan.md section 4: prevent the default only when a handler is bound to the before-navigate event, run it, then navigate from script unless `Cancel` was set. Needs the model's event name from the spec's R13.
- [ ] **T6 — Fragment links** (R10): scroll to the named control or anchor and move focus there. Not done.
- [ ] **T7 — Mobile shell** (R11): outside origins open in the system browser; belongs with WASM-012.
- [ ] **T8 — Model, Designer face, Properties, toolbox icon, `.cfrm` round trip, `tests/controls/`** (R1, R14, R15): IDE and model side.
- [ ] **T9 — Visited colours and the accessible-label property in the Properties panel** (R4): IDE side; the player already reads `VisitedColor` and `AccessibleLabel`.

## Done criteria

R2-R5 (without Cancel), R7-R9 and R12 are met in the player and run in Chrome; R6, R10, R11, R13-R15 are not.
