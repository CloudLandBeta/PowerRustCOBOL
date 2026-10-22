<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — CSS3 styling and the form stylesheet (WASM-007)

- **Status:** revised 2026-10-22 to match the spec at 2.0.29 (the operator's answers of 2026-10-22 to Q2, Q3 and Q5, and the form layers R15–R16). The player half that the 2026-10-20 plan recorded is implemented and tested natively (the property table, the cascade layers, `CssClass`, the application and per-form stylesheets, the safety check). **Not done:** the per-control inline CSS property (R2), the stylesheet files on disk and their reading by the Build (R4, Q2, Q3), the web themes (Q5), the layer guard (R15–R16), the build-time safety gate and the printed sources (R13), and everything in the IDE (R5–R7, R12) and the Preview's in-place update (R11).
- **Spec:** ./spec.md   **Date:** 2026-10-22 (first written 2026-10-20)

## 0. What changed in this revision, and why

| # | Change | Why (spec text it follows) | Code read |
|---|---|---|---|
| C1 | **The form stylesheet is a file beside the form**, `<forms folder>/<Form>.css` (`webforms/` for Web, `mobileforms/` for Mobile), found by name, not referenced from the form file. The CDATA slot in the `.cfrm` that the earlier plan carried is dropped | Q2 decided (operator, 2026-10-22): "`forms/<Form>.css`, next to the form". Web forms live in `webforms/` and mobile forms in `mobileforms/` (WASM-001 Q8), so "next to the form" puts the file in the kind's folder; the spec's literal `forms/` is read as "the forms folder" — **to confirm** (§8 Q2a). Spec §6 still says "CDATA … or in a file, Q2"; the file branch is the one decided | `crates/cobolt-compiler/src/project_kind.rs::ProjectKind::forms_folder`; `crates/cobolt-project-tools/src/tools/list.rs::web_assets` already lists "`*.css` beside the forms" |
| C2 | **A project-wide stylesheet is built now**, in its own cascade layer `pr-app` under every form's sheet | Q3 decided (operator, 2026-10-22): "add the project-wide stylesheet now". The earlier plan put it unlayered beside the form sheets, so a more specific application rule beat a form rule — that is not "under the forms' ones" | `crates/cobolt-web-player/src/dom/mod.rs::adopt` (app sheet then form sheets, both unlayered); `crates/cobolt-web-bundle/src/lib.rs` `BundleInput.stylesheet` ("WASM-007 Q3, reserved") |
| C3 | **Web themes are native web stylesheets, replaceable as a whole**: `liquid-glass`, `elegance` and `spatial` exist on the web under the desktop's names with their own CSS design; no sub-themes (no `glass_style` variants) on the web. The earlier plan's "the Spatial glass is approximate" is withdrawn | Q5 decided (operator, 2026-10-22): "Spatial / Liquid Glass is native to the web: only the names match the desktop … a web theme is replaceable as a whole (no sub-themes)" | `crates/cobolt-forms/src/theme.rs` (`LIQUID_GLASS`, `ELEGANCE`, the catalogue); `crates/cobolt-web-player/src/base_css.rs::BASE_CSS` (one plain look today) |
| C4 | **The form's generated stylesheet sits in the theme layer** (`@layer pr-theme { … }` inside the file), so it reproduces the theme (R4, AC3) without beating the properties (R8) | Without this, a stylesheet generated from the theme is unlayered developer CSS and would override every property the developer later sets — R8 would make the Properties panel useless on a new form | `crates/cobolt-web-player/src/view/style.rs` header (the layer order), `dom/sheets.rs::Sheet::set_scoped` |
| C5 | **Form layers (R15–R16) are planned**: the layer root's stacking rules are guarded in the lowest layer with `!important` (which, for important declarations, is the *highest* precedence), and Check reports a rule that targets them | R15, R16 were added to the spec at 2.0.26; the earlier plan predates them. The player has no form-layer code yet (WASM-004 plan C4) | `base_css.rs` (`[hidden] { display: none !important; }` already in `pr-base`); WASM-004 plan §1.4 |
| C6 | **The inline CSS property collides with an existing `Style`**: `ProgressBar` already has `Style` = `Continuous`/`Blocks` | R2 names the property `Style`; reusing the key would change the meaning of every saved ProgressBar. Open question Q6 (§8), with a working assumption | `crates/cobolt-forms/src/model.rs` (ProgressBar seed, `props.insert("Style", "Continuous")`); `crates/cobolt-ide/src/panels/properties.rs` (`combo_row_inline(… "Style" … ["Continuous", "Blocks"])`); `crates/cobolt-forms/src/paint.rs` (`get_prop("Style")`) |
| C7 | **The fidelity rows move to `cobolt-forms`** with WASM-004's class table (WASM-004 plan C1), so the IDE can mark and hide properties (R1) without depending on the player | R1's tooltip marking and hiding are IDE work; `cobolt-ide` does not depend on `cobolt-web-player` (`crates/cobolt-ide/Cargo.toml`) | `crates/cobolt-web-player/src/css.rs::{TABLE, fidelity_of}` |
| C8 | **AC1's test is widened** from `THEME_OWNED_PROPS` to every appearance property | AC1 says "every appearance property". The existing test (`css.rs::the_table_covers_every_theme_owned_property_of_the_model`) walks the 17 theme-owned names only; the Snackbar's `CornerRadiusTopLeft/TopRight/BottomLeft/BottomRight` (`model.rs`) have no row | `css.rs` tests; `model.rs::THEME_OWNED_PROPS` |
| C9 | **R9's unscoped fallback is removed**: a browser with neither `@scope` nor CSS nesting gets the form sheet **refused and reported**, never applied unscoped | R9: the stylesheet "shall apply inside the form's root only". `Sheet::set_scoped` today falls back to `Scoping::Unscoped`, which would let one form's sheet style every form of an SPA. Every browser of R10's matrix has nesting, so the branch is dead in the matrix and wrong outside it | `crates/cobolt-web-player/src/dom/sheets.rs::{set_scoped, Scoping}` |
| C10 | **R13 gains a build-time gate and the printed sources**: the Build runs the same safety check and prints the style/font sources the CSP must allow | R13's last clause and AC7 ("the printed policy lists the sources actually used"). Today the check runs only in the player at run time (`dom/mod.rs::styles_changed`), and the bundle copies the sheets unchecked (`cobolt-web-bundle/src/lib.rs`, `forms_cfg` / `stylesheet_cfg`) | `css.rs::css_is_safe`, `cobolt-web-bundle/src/csp.rs` (`style-src 'self'`, `font-src`) |
| C11 | A stale comment is corrected: `css.rs`'s header names the property layer `@layer pr-properties`; the code writes `pr-props` | One name in one place (`view/style.rs::wrap_layer`, `base_css.rs`) | `css.rs` lines 13–15 |

Kept unchanged from 2026-10-20: the one table (R1), CSS cascade layers rather than specificity or `!important` for the R8 order, the properties as a constructed stylesheet and never a `style` attribute, `CssClass` filtering, the type classes, constructed sheets replaced without a reload, and the Chrome 152 run recorded in §6.

## 1. Approach

### 1.1 The cascade (R8), stated once

Lowest to highest. Layers are declared in this order by the player's first sheet (`@layer pr-base, pr-theme, pr-props, pr-app;`), so the order never depends on which sheet loads first:

| # | Level (R8) | Realised as | Source |
|---|---|---|---|
| 1 | the player's structure | `@layer pr-base` | `base_css.rs` — layout furniture, `[hidden]`, the layer guard (§1.6). Not a look; not replaceable |
| 2 | the theme's defaults | `@layer pr-theme` | the web theme's stylesheet (§1.3), then the `@layer pr-theme { … }` block of a form's stylesheet (§1.4) |
| 3 | the properties | `@layer pr-props` | `view/style.rs::instance_css` (exists) |
| 4 | the project stylesheet (Q3) | `@layer pr-app` | the project file (§1.4) wrapped by the player |
| 5 | the form stylesheet, then the `CssClass` rules | **unlayered**, scoped to the form root | the form's file outside its `pr-theme` block. "CssClass rules" are rules in a stylesheet whose selector names the class; levels 5a and 5b are ordered by the ordinary cascade (specificity, order), as R8's last sentence says, not by a layer |
| 6 | the control's inline CSS (R2) | unlayered, one rule per control, adopted **after** the form sheets, selector `[data-pr-inst="n"] #NAME` scoped like the form sheet | new: `view/style.rs::inline_css` (§1.2) |

Any unlayered rule beats any layered one whatever its specificity, so levels 5–6 beat 1–4 — "CSS wins" made structural. Level 6 is not literally a `style` attribute (the page's CSP has no `'unsafe-inline'` for styles, `cobolt-web-bundle/src/csp.rs`); it wins over level 5 by coming later with an id selector, which a form-sheet rule beats only by an `!important` the developer wrote on purpose. That is a stated difference from a real inline style, recorded for the KB.

### 1.2 Per-control properties (R2, R3)

- **`CssClass`** — exists in the player (`css::class_names`): `pr-control`, the type class, then the developer's names filtered to letters, digits, `-`, `_`. The model gains the seed and the Properties panel row (web and mobile kinds only, R14).
- **Inline CSS** (spec name `Style`, see Q6) — parsed as a declaration list (`cobolt_forms::css::parse_declarations`), checked by `css_is_safe`, written as level 6 (§1.1). A declaration that does not parse is dropped and reported (R5's diagnostics run on it too, in the Properties row).
- **`id` / `data-name`** — exist; pinned by `view/contract.rs::every_control_type_has_its_class_its_id_and_its_data_name`.
- **State (R3)** — the spec now asks that the browser's `:hover`, `:focus-visible`, `:disabled`, `:checked` be usable in selectors. Each control's interactive element must therefore be the native element that carries the state (a `<button>`, an `<input type=checkbox>`) or carry the ARIA state the base theme styles (`aria-checked`, `aria-selected`, as `.pr-switch`, `.pr-tab` do). The earlier plan's open item "state classes beyond `pr-disabled`" is closed by the spec's new wording; the work left is a test that, for each control type, the element a selector would address answers those pseudo-classes (`view/contract.rs`).

### 1.3 Web themes (Q5)

- A web theme is **one complete stylesheet** in `@layer pr-theme`, named like the desktop theme (`liquid-glass`, `elegance`, `spatial`) but designed for the web; it sets the published custom properties (`--pr-accent`, `--pr-surface`, …) and the per-type look. `BASE_CSS` today mixes structure and look; it is split: structure stays in `pr-base`, the look becomes the `liquid-glass` web theme.
- The theme texts live in **`cobolt-forms`** (a `web_themes` module, no egui; each theme is a Rust string constant, as `BASE_CSS` is today — no `.css` source file enters the repository), so the player, the Build and the IDE's stylesheet generator read one copy.
- The theme is resolved as on the desktop (`form ?? project ?? liquid-glass`, `theme.rs`). **No sub-themes:** the form's `glass_style` is ignored on web and mobile kinds. **Replaceable as a whole:** a project may name its own theme file in the manifest instead of a built-in one (working assumption, Q3a) — the player then adopts that file as `pr-theme` and the built-in is not loaded.

### 1.4 The stylesheet files (R4, Q2, Q3)

- **Form:** `<forms folder>/<Form>.css`, the form's file stem. Created with the form; renamed and deleted with it. Where the IDE renames and deletes a form is to be located at `/tasks` (not read for this plan); a delete never removes a `.css` the developer edited without the confirmation of the user-code rule.
- **Its first content (R4):** a banner comment that says it was generated from theme X and may be edited, then the theme's rules for the control types the form uses inside `@layer pr-theme { … }` between marker comments carrying a hash of the block, then an empty unlayered section for the developer. The generated block reproduces the look (AC3) and stays under the properties (C4).
- **Project:** working assumption (Q3a): `[web] stylesheet = "<forms folder>/application.css"` in `cobolt.toml` (`WebSettings`, `project_kind.rs`), created with the project; a form named `application` would collide, so the Build refuses a form whose stem equals the configured file's stem.
- **Byte for byte:** the IDE reads and writes the files as bytes (no reformatting, no line-ending change); the Build copies them verbatim into the bundle (`BundleInput.stylesheet`, `FormInput.stylesheet`, existing fields). The banner rule of generated code does not apply after the first write: the file is the developer's, not regenerated (it is form data, spec §6).
- **Theme switch:** the generated block is replaced only if its hash still matches (the developer did not touch it); otherwise the IDE asks, and never rewrites developer text silently.
- **Desktop (R14):** a desktop project has no `.css` and the IDE offers no button; the model properties are seeded only on web and mobile kinds.

### 1.5 The IDE: the button, the modal, the markings (R1, R5–R7, R12)

- **Properties pane, form selected (web/mobile kinds):** a button "Stylesheet…" opens `panels/stylesheet_modal.rs`, built on `panels/leaderboard_modal.rs`'s pattern: an explicit `size`, opening size, min/max stops, children laid out from the size, one grip as the only writer (R7). Read the `egui-resize-guardian` and `egui-paint-regressions` notes first.
- **Editor:** a multi-line code editor with CSS colouring from a small Rust tokenizer (comments, strings, at-rules, selectors, properties, values); line numbers; the diagnostics list under it, each with its line, clicking jumps to it.
- **Diagnostics (R5):** a new `cobolt_forms::stylesheet_lint` module (no egui, so Check and the agent tools reuse it): unbalanced braces / unterminated strings or comments (error); unknown property (warning, against a Rust table of CSS property names); a value that does not parse (warning: colours and lengths through `cobolt_forms::css::{color, length}`); a feature outside the project's browser matrix (warning, from `WebSettings.browsers`); `css_is_safe` refusals (error); R16 violations (error, §1.6). Every diagnostic carries a line — `cobolt_forms::css::Stylesheet` keeps no positions, so the lint has its own line-aware scanner rather than changing the Viewer's parser.
- **Buttons:** Apply (writes the file, pushes it to an open Preview), Cancel (discards the buffer; asks if it changed), Revert to theme default (asks; replaces the **buffer** only; Ctrl+Z in the editor and Cancel both undo it; nothing reaches disk until Apply — R6). After Apply the previous file text is kept as the modal's "Undo last apply" for the session.
- **Markings (R1):** the Properties panel reads the fidelity rows from `cobolt-forms` (C7): an *approximate* property's tooltip says so; a *not on the web* property is hidden on web/mobile forms (WASM-004 R6, its plan C2); the value still round-trips.
- **Overrides (R12):** for the selected control, the IDE cascades the project sheet and the form sheet (`cobolt_forms::css::Stylesheet::parse` + `cascade` with an `ElementRef` path of form root → control: its id, `pr-control`, type class, `CssClass` names) plus its inline CSS, and maps each winning declaration back to the property it overrides through the table's CSS column. The property row shows an "overridden by CSS (file:line)" mark. The designer canvas draws a small corner badge on a control that has inline CSS or a matching class rule ("CSS the designer does not paint"), and does not try to paint the CSS.
- **i18n:** every label, button, confirmation, diagnostic text and tooltip marking is a `Tr` field in `crates/cobolt-ide/src/i18n.rs` in all six languages; the lint returns codes plus arguments, the IDE formats them through `Tr`.

### 1.6 Layers (R15, R16)

- The layer element WASM-004 §1.4 creates (`position: absolute; inset: 0; isolation: isolate; z-index: k`, clipped by the form root with `overflow: hidden` and its `border-radius`) carries a class from the contract, proposed **`pr-form-layer`** — `.pr-layer` is already the player's popup furniture (`base_css.rs`, `dom/mod.rs`) and must not be reused (Q4).
- **Guard at run time:** `pr-base` declares `position`, `inset`, `z-index`, `isolation`, `overflow` and `pointer-events` of `.pr-form-layer` and of the form root with `!important`. In the cascade, important declarations in an *earlier* layer beat important ones in later layers and unlayered ones, so no developer rule can move or unstack a layer. `display`/`visibility` are guarded the same way for the hidden state (`[hidden]` is already `display: none !important` in `pr-base`); `inert` is an attribute, which CSS cannot set.
- **Guard at Check (R16, AC10):** `stylesheet_lint` reports a rule whose selector can match a layer root or a control's layer container and that sets one of `position`, `z-index`, `display`, `visibility`, `pointer-events` (and an `inert`-like `interactivity` property), with its line. The Properties panel marks it as an override (R12).

### 1.7 Safety (R13) and the Preview (R11)

- **Build gate:** the Build (the `WebBuildBackend` implementation, still `NotYetLinked` in `crates/cobolt-project-tools/src/web/mod.rs`) runs `stylesheet_lint` on every sheet; an error stops the Build with file and line. `css_is_safe` moves beside the lint in `cobolt-forms` (the player calls the same function) so the two never disagree.
- **Printed policy:** the Build report lists, for `style-src` and `font-src`, the origins the sheets actually use (from their `url()`s and `@font-face` sources), next to the CSP the bundle writes (`csp.rs`).
- **Run time:** the player keeps its check; a refused form sheet now shows the same banner as a refused project sheet (it only logs to the console today).
- **Preview (R11, AC8):** the dev server already watches the bundle (`devserver.rs`, `WATCH_INTERVAL` 150 ms, `/__prc/version`), but its live reload reloads the page. For a change that touched only `.css` files the page script calls `player.set_stylesheet` / `set_form_stylesheet` with the new text instead of reloading — the player side exists (`api.rs`). The protocol change is WASM-008's; this plan sets the **budget: 500 ms from Apply to the changed computed style** (watch interval + fetch + `replaceSync`), measured and printed.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/`: `base_css.rs` (split into structure + the default theme; layer order statement; layer guard), `css.rs` (header fix C11; rows move to `cobolt-forms`, CSS functions stay), `view/style.rs` (`inline_css`), `view/contract.rs` (the contract test widened: pseudo-class targets, `pr-form-layer`, the layer order), `dom/sheets.rs` (no unscoped fallback; `pr-app` wrapper), `dom/mod.rs` (`adopt` order: base, theme, instances, app, forms, inline; banner for a refused form sheet), `api.rs` (`set_theme`).
- `crates/cobolt-forms/src/`: `web.rs` (WASM-004 C1, gains the fidelity rows), new `web_themes.rs`, new `stylesheet_lint.rs` (lint + `css_is_safe`), `model.rs` (seeds of `CssClass` and the inline-CSS property for web/mobile kinds only; ProgressBar untouched, Q6).
- `crates/cobolt-web-bundle/src/lib.rs` (sheets checked; theme stylesheet; sources reported), `csp.rs` (sources from the sheets).
- `crates/cobolt-compiler/src/project_kind.rs` (`WebSettings`: `stylesheet`, `theme_file`), `crates/cobolt-project-tools/` (create: the project and form `.css`; Build backend reads them; `tools/list.rs` already lists them).
- `crates/cobolt-ide/src/`: `panels/properties.rs` (button, rows, markings, override marks), new `panels/stylesheet_modal.rs`, the designer canvas badge, `i18n.rs`, form rename/delete handling of the sibling `.css`.
- System KB: `crates/cobolt-compiler/src/lib.rs` (`property_reference_for`, `control_structure`) and the regenerated `assets/knowledge/chunked.data`.

## 3. Data / model changes

- `.cfrm` / `.wcfrm` / `.mcfrm` XML: **no format change** — the stylesheet is a sibling file (C1); the two control properties are ordinary `PropValue::String` entries, absent from desktop forms.
- `cobolt.toml` `[web]`: `stylesheet` (path, default `<forms folder>/application.css`) and `theme_file` (optional, a whole-theme replacement); both `#[serde(default)]`, so existing manifests load unchanged.
- The bundle: `forms/<Form>.css` (exists), `stylesheet.css` (exists), new `theme.css`; manifest key `theme`.

## 4. Key decisions & alternatives

- **Layers, not specificity** (kept). Rejected: `!important` for ordering; a `style` attribute (needs `'unsafe-inline'`).
- **The generated stylesheet lives in `pr-theme`** (C4). Rejected: generating it unlayered (overrides every property from the first minute); generating it as comments (does not reproduce the look, AC3 fails).
- **Project sheet in `pr-app`** (C2). Rejected: unlayered beside the form sheets (a specific application rule beats a form rule).
- **`!important` in the lowest layer as the layer guard** (C5). This is the one deliberate `!important`: it protects the player's invariants, not the cascade order. Rejected: relying on Check alone (CSS wins at run time, so a rule the build missed would break layering).
- **One copy of the fidelity rows, the themes and the safety check in `cobolt-forms`** (C7, §1.3, §1.7). Rejected: the IDE depending on the player crate (a `wasm32` crate with web-sys).
- **Refuse rather than apply unscoped** (C9).
- **Class names and custom properties pinned by `view/contract.rs`** — proposed as the contract; **not** declared shipped until the operator confirms Q4.

## 5. Risks & mitigations

- **`@layer` inside `@scope` and inside a nesting wrapper** (the form sheet's `pr-theme` block is wrapped by `set_scoped`): supported in the specs, not yet run here. → A Chrome/Safari/Firefox check is the first task; if one engine drops it, the player lifts the `pr-theme` block out of the form text before scoping (it is delimited by markers).
- **R12's override marks are an approximation:** the IDE's cascade (`cobolt_forms::css`) evaluates `@media` at a fixed desktop width (`MEDIA_WIDTH_PX`), ignores layers and `@scope`, and knows no pseudo-class state. → Marks say "may be overridden" for rules under `@media`/`@container` or with pseudo-classes; the Preview is the truth.
- **Theme switch with a hand-edited generated block** → hash in the marker; ask, never rewrite (user code is sacred).
- **The inline property's name** (Q6) → nothing is seeded until the operator answers or `/tasks` adopts the working assumption explicitly.
- **AC3's pixel comparison** depends on font rasterisation → compare in the same browser run, tolerance: at most 1 % of pixels differing by more than 8/255 in any channel; report the measured figure.
- **The Build backend is not linked yet** (`NotYetLinked`) → the stylesheet reading and the gate are written against `WebBuildRequest` and tested with the bundle's own test support (`cobolt-web-bundle/tests/support`) until WASM-002 links the backend.

## 6. Test strategy

Run natively: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib`; `cargo test -p cobolt-forms` (the lint, the themes, the rows); `cargo test -p cobolt-web-bundle`; `cargo test -p cobolt-ide --bin cobolt-ide` (modal, rows); each prints what it measured.

| AC | Test | Asserts / reports |
|---|---|---|
| AC1 | `cobolt-forms` `web` test (C8) | every appearance property of the seeds — universal and per type, including the Snackbar corner radii and the chart border properties — has a row with a marking; prints the counts per marking |
| AC2 | headless Chrome on the gallery bundle | each *exact* property set to a distinct value gives the expected `getComputedStyle`; prints property → expected → got |
| AC3 | headless Chrome | a new form with its generated sheet vs the same form with properties only, per built-in web theme: differing-pixel ratio under the §5 tolerance; prints the ratio |
| AC4 | IDE test of `stylesheet_modal` + byte round-trip | button opens; a bad sheet lists diagnostics with lines; Apply writes; Cancel and Revert (with the confirmation) restore; a file with CRLF, comments and odd spacing read and written is byte-identical |
| AC5 | Chrome, a fixture setting `background-color` at each of the six levels | the visible value is the highest level's; removing levels one by one from the top steps down the table of §1.1 |
| AC6 | Chrome, host page with `button { color: red }` and a sheet with `button { … }` | neither crosses the shadow boundary; `--pr-accent` set on the mount element changes the accent |
| AC7 | `stylesheet_lint` unit tests + bundle test | a remote `url()` refused unless allowed; the Build report lists exactly the sources used |
| AC8 | Preview run (with WASM-008) | Apply → changed computed style; time printed against the 500 ms budget |
| AC9 | IDE frame test | the modal's rectangle is the same over 120 frames in each of the six languages, and keeps the dragged size after a grip drag |
| AC10 | `stylesheet_lint` unit test | `.pr-form-layer { z-index: 5 }` and `#LAYER-1 { pointer-events: none }` reported with their lines; plus a Chrome check that the guard holds when such a rule is applied anyway |

Already run (2026-10-20, kept): Chrome 152, `player.set_stylesheet` changed a button's background twice with no reload; a sheet with `@import` was refused with a visible message and the previous sheet stayed; an empty sheet restored the default. `set_form_stylesheet` has not been driven in a browser.

## 7. Steering compliance

- [ ] i18n ×6: modal, buttons, confirmations, diagnostics, tooltip markings, override marks (`i18n.rs`).
- [x] Generated-code contract: the stylesheet is form data written once from the theme, then the developer's; no generated COBOL changes.
- [x] `README.md` and `docs/` untouched (2.x.x); what 2.x.x says about itself stays in `specs/`.
- [ ] System KB: `CssClass`, the inline-CSS property, the form/project stylesheets and the cascade in `property_reference_for` / `control_structure`; `chunked.data` regenerated, freshness test green.
- [ ] Self-resizing guard (R7, AC9), `leaderboard_modal.rs` as the reference.
- [x] PRIME DIRECTIVE: themes and the CSS-property table are Rust constants; the only `.css` files are the developer's project files, created by the IDE, not committed sources.
- [x] User code is sacred: Revert asks and is undoable; theme switch never rewrites edited text; deleting a form asks before deleting an edited `.css`.
- [x] Interpreter/binary parity: not affected — styling is the player's; the desktop runtime and binary are unchanged (R14).

## 8. Open questions

- **Q1** — decided (CSS wins), realised by §1.1.
- **Q2** — decided (a file beside the form). **Q2a, to confirm:** the spec's `forms/<Form>.css` is read as "in the form's folder" (`webforms/`, `mobileforms/`), because that is where web and mobile forms live (WASM-001 Q8).
- **Q3** — decided (project stylesheet now). **Q3a, to confirm:** its location (`[web] stylesheet`, default `<forms folder>/application.css`) and the whole-theme replacement file (`[web] theme_file`) are working assumptions.
- **Q4 — Not answered.** Working assumption: the names in `view/contract.rs` (`pr-control`, `pr-<type>`, `pr-form`, the thirteen published custom properties) plus `pr-form-layer`, pinned by test, are the proposal; they are not a published contract until the operator says so.
- **Q5** — decided (native web themes, no sub-themes), realised by §1.3.
- **Q6 — new, not answered:** the inline CSS property cannot be `Style` without colliding with `ProgressBar.Style`. Working assumption for `/tasks`: the model key is **`CssStyle`** (beside `CssClass`), shown in the Properties panel as "Style (CSS)"; the alternatives are renaming ProgressBar's property with a load-time migration (touches desktop forms, against R14) or giving ProgressBar alone no inline CSS. The operator decides.
