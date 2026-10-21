<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — CSS3 styling and the form stylesheet (WASM-007)

- **Status:** the player half is implemented and tested natively (the property table, the cascade layers, `CssClass`, the application and per-form stylesheets, the safety check); **the per-control `Style` property (R2), the state classes beyond `pr-disabled` (R3) and everything in the IDE (R5-R7, R12) are not done**
- **Spec:** ./spec.md   **Date:** 2026-10-20

## 1. Approach

- **R1 — one table.** `crates/cobolt-web-player/src/css.rs::TABLE` maps every appearance property to CSS3 declarations and marks each entry **exact**, **approximate** or **not on the web** (`fidelity_of`). What the desktop painter does and CSS cannot (liquid glass, the perspective tilt of a whole control, the operating-system blur) is listed there as approximate or not on the web and stays editable, so a form round-trips between kinds (Q3 of WASM-004).
- **R8 — CSS wins, in one place.** The cascade, lowest to highest, is written once (`css.rs` header, `base_css.rs` header) and built from CSS cascade layers, in the page's constructed stylesheets only:
  1. `@layer pr-base` — the player's base stylesheet (`base_css.rs`): the theme's defaults, the public class names and the custom properties (`--pr-accent`, `--pr-surface`, ...).
  2. `@layer pr-props` — the control's properties (`view/style.rs::instance_css`): what the Properties panel says.
  3. **unlayered** — the application stylesheet and each form's own stylesheet (`set_stylesheet`, `set_form_stylesheet`), which therefore win whatever their specificity.
  4. the control's `Style` (R2) — **not implemented** (below).
- **R3 — classes.** Each element carries `pr-control`, a class for its type (`pr-button`, `pr-textbox`, `pr-datetimepicker`, ...; `css::type_class`, pinned by `view/contract.rs` as the public contract of Q4) and the developer's `CssClass` names (filtered to letters, digits, `-` and `_`). The state class that exists is `pr-disabled` (with `aria-disabled`); `pr-selected` on tree nodes. The rest of the state classes the spec names (focus, hover as classes, invalid, ...) are **not** written: the browser's own `:focus-visible`, `:hover` and `:invalid` pseudo-classes apply, which covers the stylesheet author's need but not the spec's list.
- **R4, R9, R11 — stylesheets, scoped, live.** The application stylesheet and each form's are constructed `CSSStyleSheet`s; a form's is scoped to that form's root (`set_scoped`), the page's own CSS never reaches in (the shadow root, WASM-002) and the player's CSS never leaks out. Replacing either while the application runs needs no reload (`set_stylesheet`, `set_form_stylesheet` re-apply the sheets): that is the player half of R11 and of WASM-016 R16.
- **R13 — no remote resource unless allowed.** `css::css_is_safe(css, allowed_origins)` refuses `@import`, `expression()`, `javascript:` and any `url()` that is not the project's own asset, a `data:` URI or an allowed origin; the page's CSP (`img-src`, `font-src`, `connect-src` built from the same origins) is the second wall. A refused stylesheet is not applied and the reason is shown (the application's) or logged (a form's).
- **R10 — evergreen CSS.** Cascade layers, constructed stylesheets, `:focus-visible`, `inset`, `contain`, `@layer`-scoped rules: current Chrome, Safari and Firefox, and the platform web views that follow them. Measured on Chrome 152 only.
- **Nothing writes a `style` attribute.** The page's CSP has no `'unsafe-inline'` for styles, so every declaration reaches the page through the CSSOM (constructed sheets and `insertRule`); `tests/sinks.rs` and `no_string_execution` keep it so.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/{css,base_css,layout_css}.rs`, `src/view/{style,contract}.rs`, `src/dom/{sheets,mod}.rs` (constructed sheets, the scoped sheet, the safety gate), `src/api.rs` (`set_stylesheet`, `set_form_stylesheet`).
- `crates/cobolt-web-bundle` — writes the application and form stylesheets into the bundle (`FormInput.stylesheet`, `BundleInput.stylesheet`) and builds the CSP from the allowed origins.
- Not here: the form model's `CssClass` / `Style` properties and the stylesheet storage in the `.cfrm` (R4's "round-tripped byte for byte", Q2), the Properties pane button, the modal editor (R5-R7), the designer's marking of controls with CSS overrides (R12).

## 3. Data / model changes

None in the player. The model needs `CssClass` (read today) and `Style` (not read), and a stylesheet slot on the form (inside the `.cfrm` in a CDATA section was recommended in Q2; the bundle takes it as a field of `FormInput`).

## 4. Key decisions & alternatives

- **Decision: layers, not specificity.** `@layer` gives the cascade R8 asks for without `!important` and without the developer fighting selector weight. Rejected: inline declarations (a `style` attribute needs `'unsafe-inline'`) and `!important`.
- **Decision: the properties are a stylesheet, not per-element inline styles.** One rule per control (`[data-pr-inst="n"] ...` selectors from `view/style.rs`), written when the control changes. Why: CSP, and the developer's stylesheet can override it.
- **Decision: class names and custom properties are the contract** (Q4): `view/contract.rs` pins them so a rename is a red test.
- **Not decided here (Q5, the visual bar):** how close to the desktop's Spatial look the default CSS gets; `base_css.rs` is a plain readable default, the Spatial glass is approximate.

## 5. Risks & mitigations

- Risk: a stylesheet reaches a remote address. → `css_is_safe` and the CSP, both tested.
- Risk: a form's stylesheet affects another form or the host page. → scoped to the form's root and shadow-rooted.

## 6. Test strategy

- `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib css` and `view::style`, `view::contract`: the table, the declarations of each property group, the layer wrapper, `css_is_safe` (allowed asset, `data:`, an allowed origin, a refused origin, `@import`, `expression()`, `javascript:`), `class_names` (a `CssClass` with a bad character keeps the good names), the contract names.
- Chrome 152 (gallery bundle, driving `player.set_stylesheet` from the page): a button's background went from the default `rgb(58, 62, 71)` to `rgb(200, 10, 20)` and then to `rgb(10, 200, 20)` with no reload; a sheet with `@import` was refused with a visible message ("the stylesheet uses `@import`, which is refused") and the previous one stayed applied; an empty sheet restored the default. `set_form_stylesheet` was not driven in the browser.
- Not run: the Preview (WASM-008) showing a change, the designer markers, the modal.

## 7. Steering compliance

- [x] No secret or remote resource reaches a bundle through a stylesheet.
- [x] `README.md` and `docs/` untouched.
- [ ] `Style`, state classes, stylesheet modal, Revert-to-default with confirmation and undo, the modal's no-self-resize rule (R7), the designer's CSS marker: IDE side; the modal must follow `panels/leaderboard_modal.rs` as the reference.
