<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — CSS3 styling and the form stylesheet (WASM-007)

- **Status:** in progress — the player half is done; `Style`, the state classes and the IDE half are not
- **Plan:** ./plan.md   **Date:** 2026-10-20

- [x] **T1 — The property-to-CSS table** (R1): `src/css.rs::TABLE`, `fidelity_of`, the declarations (`appearance`, `border`, `text`, `box_shadow`, gradients).
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib css` green.
- [x] **T2 — Cascade layers** (R8): `@layer pr-base`, `@layer pr-props`, then the unlayered developer sheets; `view/style.rs::wrap_layer`, `base_css.rs`.
- [x] **T3 — `CssClass` and the type classes** (R2 in part, R3 in part): `css::class_names`, `type_class`; the public names pinned by `view/contract.rs`.
- [x] **T4 — Application and per-form stylesheets, scoped, replaceable without a reload** (R4, R9, R11 player half): `set_stylesheet`, `set_form_stylesheet`; constructed sheets; scoped to the form's root.
- [x] **T5 — Stylesheet safety** (R13): `css::css_is_safe`; the CSP from the same origins.
- [x] **T6 — No inline style, no `unsafe-inline`** (R8, steering): everything through the CSSOM; `tests/sinks.rs`, `csp` tests.
- [ ] **T7 — The per-control `Style` property** (R2): **not done.** It should be the last, highest rule for the control (after `CssClass` rules), parsed as a declaration list, checked by `css_is_safe`, applied as a constructed-sheet rule for the control's selector (never a `style` attribute).
- [ ] **T8 — The state classes of R3** (focus, hover, invalid, ... as classes): not written; the browser's pseudo-classes cover the author's need but not the spec's list. `pr-disabled` and `pr-selected` exist.
- [ ] **T9 — The stylesheet in the `.cfrm`, byte for byte, and the Properties pane button and modal** (R4, R5, R6, R7, AC): IDE and model side.
- [ ] **T10 — Designer marks a control with CSS overrides it cannot paint** (R12): IDE side.
- [ ] **T11 — Preview shows a change without a rebuild** (R11, WASM-008): the player's side exists; the Preview is WASM-008.
- [ ] **T12 — Desktop unchanged** (R14): nothing in this crate touches a desktop form.

## Done criteria

R1, R4 (player side), R8 (without `Style`), R9, R10 (Chrome 152), R13 are met in the player with tests. R2's `Style`, R3's state classes, R5-R7, R11's Preview, R12 and R14 are not.
