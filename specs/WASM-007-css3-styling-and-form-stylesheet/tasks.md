<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — CSS3 styling and the form stylesheet (WASM-007)

- **Status:** in progress — the player half (T1–T6) is done and tested; the revised plan of 2026-10-22 (the layer order with `pr-theme`/`pr-app`, the inline CSS property, the stylesheet files, the web themes, the lint and build gate, the layer guard, the IDE half, the Preview's in-place update) is not started
- **Plan:** ./plan.md   **Date:** 2026-10-22 (first written 2026-10-20; revised to match the plan at 2.0.29)

Ordered, small, independently verifiable tasks. Each names the files it touches, the requirements it satisfies and how it is verified. Native tests print what they measured. Browser runs use Chrome 152 on 127.0.0.1 only (plus Safari and Firefox where a task says so); their numbers are written here when they are run. Browser tooling runs outside the repository (PRIME DIRECTIVE); the repository holds only the Rust fixtures and tests.

Commands used below:
- forms: `cargo test -p cobolt-forms --features render <filter>`
- player: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib <filter>` (the player is outside the workspace)
- bundle: `cargo test -p cobolt-web-bundle <filter>`; project tools: `cargo test -p cobolt-project-tools <filter>`; compiler: `cargo test -p cobolt-compiler <filter>`
- IDE: `cargo test -p cobolt-ide --bin cobolt-ide <filter>`

Working assumptions (open questions; the operator may rule otherwise, and each is one constant or one rule to change):
- **Q2a — to confirm.** "`forms/<Form>.css`" is read as *the form's own folder*: `webforms/<Form>.css` or `mobileforms/<Form>.css` (`crates/cobolt-compiler/src/project_kind.rs::ProjectKind::forms_folder`).
- **Q3a — to confirm.** The project stylesheet is `[web] stylesheet` in `cobolt.toml`, default `<forms folder>/application.css`; a whole-theme replacement is `[web] theme_file` (optional). Both `#[serde(default)]` on `WebSettings` (`project_kind.rs:273`); a Mobile project gets the same two keys on `MobileSettings` (`project_kind.rs:320`) — the plan names only `[web]`, so the `[mobile]` keys are part of this assumption.
- **Q4 — not answered.** The class names (`pr-control`, `pr-<type>`, `pr-form`, the new `pr-form-layer`) and the published custom properties (`view/contract.rs::CUSTOM_PROPERTIES`) are pinned by test as a *proposal*; nothing here calls them a shipped contract.
- **Q6 — not answered.** The inline CSS property is the model key **`CssStyle`**, shown as "Style (CSS)" (R2 names it `Style`, which `ProgressBar` already uses: `crates/cobolt-forms/src/model.rs:5462`, `crates/cobolt-ide/src/panels/properties.rs:8230`, `crates/cobolt-forms/src/paint.rs:4828`). ProgressBar is untouched.
- **R14 reconciled with plan §1.2.** `CssClass` and `CssStyle` are **not** seeded in `Control::new` (it serves desktop forms too); the Properties panel shows their rows on Web and Mobile kinds only and the value is written to the form only when non-empty, so a desktop form never carries them. To confirm at review; the plan's "the model gains the seed" is read this way.
- **Dependencies on other specs.** The fidelity rows move to `crates/cobolt-forms/src/web.rs` in **WASM-004 T12/T13**; hiding `NotOnWeb` and marking `Approximate` in the Properties panel is **WASM-004 T27** (R1's IDE half is done there, not duplicated here). The form root and the layer elements are **WASM-004 T18**. The Preview's protocol is **WASM-008**. The Build backend is `NotYetLinked` (`crates/cobolt-project-tools/src/web/mod.rs:258`) until WASM-002 links it; the gate is written against `WebBuildRequest` and tested with the bundle's own support until then.

## Done (kept from 2026-10-20, with their evidence)

- [x] **T1 — The property-to-CSS table** (R1): `crates/cobolt-web-player/src/css.rs::TABLE`, `fidelity_of`, the declarations (`appearance`, `border`, `text`, `box_shadow`, `background`).
  - Verified: player `css` green; `the_table_covers_every_theme_owned_property_of_the_model` (walks `THEME_OWNED_PROPS` only — T12 widens it, plan C8).
- [x] **T2 — Cascade layers** (R8): `@layer pr-base` (`base_css.rs`), `@layer pr-props` (`view/style.rs::wrap_layer`), then the unlayered developer sheets. *(T8 adds `pr-theme` and `pr-app` and states the order once.)*
- [x] **T3 — `CssClass` and the type classes** (R2 in part, R3 in part): `css::class_names`, `css::type_class`; pinned by `view/contract.rs::every_control_type_has_its_class_its_id_and_its_data_name` and `css_class_adds_the_developers_classes_after_the_type_class_and_drops_what_is_not_a_class_name`.
- [x] **T4 — Application and per-form stylesheets, scoped, replaceable without a reload** (R4, R9, R11 player half): `api.rs::set_stylesheet` (`:1028`), `set_form_stylesheet` (`:1034`); constructed sheets (`dom/sheets.rs::Sheet::set`, `set_scoped`). Chrome 152: `player.set_stylesheet` changed a button's background twice with no reload; an empty sheet restored the default. `set_form_stylesheet` not yet driven in a browser (T23).
- [x] **T5 — Stylesheet safety at run time** (R13 in part): `css::css_is_safe` (`:378`), called by `dom/mod.rs::styles_changed`; the CSP from the same origins (`cobolt-web-bundle/src/csp.rs`). Chrome 152: a sheet with `@import` was refused with a visible message and the previous sheet stayed.
- [x] **T6 — No inline style, no `unsafe-inline`** (R8, steering): everything through the CSSOM; `tests/sinks.rs`, the `csp` tests.

## Player corrections (R8, R9, R13)

- [x] **T7 — Browser probe: `@layer` inside `@scope` and inside a nesting wrapper** (R8, R9; plan §5 first risk)
  - Files: none in the repository (a probe page built by tooling outside it, using the player's `base_css::scoped` / `base_css::nested` output for a form sheet that contains `@layer pr-theme { … }`).
  - Verify: in Chrome 152, Safari and Firefox, a rule inside the form sheet's `pr-theme` block loses to a `pr-props` rule and an unlayered rule of the same sheet wins; results per engine written here. If one engine drops the block, T8 lifts the marked `pr-theme` block out of the form text before scoping (plan §5) — the decision is recorded here before T8 starts.
  - **Run 2026-10-30 (partial):** probe page outside the repository, constructed sheets adopted in the order `@layer pr-base, pr-theme, pr-props, pr-app;` → `@layer pr-props { … }` → the form sheet wrapped as `base_css::scoped` and as `base_css::nested` emit it.
    - **Chrome 156** (headless, the installed Chrome; 152 is no longer on this machine), both wrappers identical: the `pr-theme` rule applies where nothing beats it (`color` rgb(9, 9, 9)); `pr-props` beats it (`background-color` rgb(2, 2, 2)); the sheet's unlayered rule beats `pr-props` (`color` rgb(3, 3, 3)). **Pass.**
    - **Safari 18.5** (run 2026-10-31 through `safaridriver`, after the operator enabled remote automation), both wrappers identical to Chrome: the `pr-theme` rule applies where nothing beats it (`color` rgb(9, 9, 9)); `pr-props` beats it (`background-color` rgb(2, 2, 2)); the sheet's unlayered rule beats `pr-props` (`color` rgb(3, 3, 3)).
    - **Firefox:** not run — **operator, 2026-10-31: do not spend time on Firefox**; the probe closes on Chrome and Safari.
    - **Decided for T8 (2026-10-31):** Chrome and Safari keep the `pr-theme` block inside both wrappers, so T8 keeps it inside the scope wrapper; nothing is lifted out.

- [ ] **T8 — The layer order stated once; the project sheet in `pr-app`** (R8, Q3; plan §1.1, C2)
  - Files: `crates/cobolt-web-player/src/base_css.rs` (first statement `@layer pr-base, pr-theme, pr-props, pr-app;`), `dom/sheets.rs` (a `pr-app` wrapper for the application sheet), `dom/mod.rs::adopt` (`:214`: base, theme, instances, app, forms, inline), `view/style.rs` and `css.rs` headers (C11: `pr-properties` → `pr-props`, the one name `wrap_layer` writes).
  - Verify: player `base_css` — the base sheet starts with the order statement; `dom` — the adopted list is in the order above; `css.rs` no longer contains `pr-properties` (asserted by a test that greps the header). Existing player tests stay green.

- [ ] **T9 — A form sheet that cannot be scoped is refused, never applied unscoped** (R9, R13; plan C9, §1.7 run time)
  - Files: `crates/cobolt-web-player/src/dom/sheets.rs` (`set_scoped` `:122` — the `self.set(css); Scoping::Unscoped` fallback replaced by `Scoping::Refused`, the sheet left empty), `dom/mod.rs::styles_changed` (`:771`; a refused form sheet shows the same banner as a refused application sheet, not only `console::error_1` at `:796`).
  - Verify: player — `Scoping` has no `Unscoped`; a form sheet refused by `css_is_safe` and one refused for scoping both raise the banner and leave the sheet empty. Chrome (T23's session): a form sheet with `@import` shows the banner.

## Per-control CSS (R2, R3)

- [ ] **T10 — The inline CSS property, level 6** (R2, R8; Q6 working assumption)
  - Files: `crates/cobolt-web-player/src/view/style.rs` (new `inline_css(inst, ctls)`), `dom/mod.rs` (one inline sheet adopted after the form sheets), `css.rs` (`CssStyle` read; not in `TABLE`, it is not an appearance property).
  - Do: parse `CssStyle` as a declaration list (`cobolt_forms::css::parse_declarations`), check with `css_is_safe`, write one unlayered rule per control, selector `[data-pr-inst="n"] #NAME`, scoped like the form sheet; a declaration that does not parse is dropped and reported; never a `style` attribute.
  - Verify: player `style::inline_` — the rule is unlayered, comes after the form sheets in the adopted list, carries only the declarations that parsed; a hostile value (`}` / `</style>` / `url(http://x)`) cannot open a rule or load anything; `tests/sinks.rs` still finds no `style` attribute sink.

- [ ] **T11 — The browser's state pseudo-classes work on every control type** (R3; plan §1.2)
  - Files: `crates/cobolt-web-player/src/view/contract.rs` (`the_states_a_stylesheet_may_select_are_the_browsers_own_pseudo_classes` `:133`, today Button and CheckBox only).
  - Do: for every interactive type in the classification, the element a selector addresses is the native element carrying the state (`<button>`, `<input>`, `<select>`, `<textarea>`) or carries the ARIA state the base styles (`aria-checked`, `aria-selected`); a type that has neither is fixed in `view/controls.rs` / `view/data.rs`.
  - Verify: player `contract` — the widened test lists per type the element and which of `:hover`, `:focus-visible`, `:disabled`, `:checked` (or its ARIA twin) it answers; prints the list.

## Shared definitions in `cobolt-forms` (R1, R5, R13, R16)

- [ ] **T12 — AC1 widened to every appearance property** (R1, AC1; plan C7, C8; after WASM-004 T13)
  - Files: `crates/cobolt-forms/src/web.rs` (the fidelity rows, moved there by WASM-004 T13; rows added here), its test module.
  - Do: rows for every appearance property the seeds create, universal and per type — including the Snackbar's `CornerRadiusTopLeft/TopRight/BottomLeft/BottomRight` (`model.rs:6485`) and the charts' `BorderGradient*`, `BorderBlur`, `BorderTransparency` — each `Exact | Approximate | NotOnWeb`.
  - Verify: forms `web::every_appearance_property_has_a_row` — builds a `Control` of every `ControlType::ALL`, collects the appearance properties of its seeds, fails on any without a row; prints the counts per marking (**AC1**).

- [ ] **T13 — `stylesheet_lint`: the safety check and the diagnostics, with lines** (R5, R10, R13, R16; AC7 and AC10 natively; plan §1.5, §1.6, §1.7)
  - Files: `crates/cobolt-forms/src/stylesheet_lint.rs` (new, no egui), `crates/cobolt-forms/src/lib.rs` (`pub mod stylesheet_lint`), `crates/cobolt-web-player/src/css.rs` (`css_is_safe` becomes a call into the lint's function, so the two never disagree).
  - Do: a line-aware scanner (the Viewer's `cobolt_forms::css::Stylesheet` keeps no positions and is not changed). Diagnostics as codes plus arguments plus a line: unbalanced braces, unterminated string or comment (error); unknown property against a Rust table of CSS property names (warning); a value that does not parse — colours and lengths through `cobolt_forms::css::{color, length}` (warning); a feature outside the project's `BrowserMatrix` (warning; `@media`, container queries, `@font-face` and `env(safe-area-inset-*)` are inside it, R10); `url()` outside the project's assets, `data:` and the allowed origins, `@import`, `expression(`, `javascript:`, `behavior:` (error, R13); a rule whose selector can match `.pr-form-layer`, `.pr-form` or a control's layer container and that sets `position`, `z-index`, `display`, `visibility`, `pointer-events` or `interactivity` (error, R16); `]]>` is not checked (the sheet is a file, C1). `fn sources(css) -> (style origins, font origins)` for the printed policy.
  - Verify: forms `stylesheet_lint` — one test per diagnostic with its expected line; **AC10**: `.pr-form-layer { z-index: 5 }` and `#LAYER-1 { pointer-events: none }` reported with their lines; **AC7 (native)**: `url(https://cdn.example/x.woff2)` refused, accepted once the origin is allowed, and `sources` returns exactly it; the player's old `css_is_safe` tests pass unchanged against the moved function.

- [ ] **T14 — The web themes, and the form stylesheet generated from one** (R4, R9, Q5; plan §1.3, §1.4, C3, C4)
  - Files: `crates/cobolt-forms/src/web_themes.rs` (new, no egui; Rust string constants, no `.css` source in the repository), `crates/cobolt-forms/src/lib.rs`, `crates/cobolt-web-player/src/base_css.rs` (`BASE_CSS` split: structure stays in `pr-base`; the look moves to the `liquid-glass` web theme), `api.rs` (`set_theme`).
  - Do: three complete themes named as the desktop's (`theme.rs::LIQUID_GLASS`, `ELEGANCE`, `SPATIAL`), each one stylesheet in `@layer pr-theme` defining the published custom properties (`--pr-accent`, `--pr-surface`, the text colour, the radius, … — `CUSTOM_PROPERTIES`) and the per-type look; no sub-themes (`glass_style` ignored on Web/Mobile). `fn theme_css(id)`, resolved as `theme.rs::resolve_theme_id`; `fn form_stylesheet(theme, used_types) -> String`: a banner comment (generated from theme X, yours to edit), the theme's rules for the types the form uses inside `@layer pr-theme { … }` between marker comments carrying a hash of the block, then an empty unlayered section; `fn generated_block_untouched(text) -> bool`.
  - Verify: forms `web_themes` — each theme defines every published custom property and only reads defined ones (the check `view/contract.rs::the_custom_properties_a_host_page_can_set_are_the_published_list` makes, moved or repeated); the generated sheet passes `stylesheet_lint` with no error; editing one character of the block flips `generated_block_untouched`. Player — `base_css` keeps the structure classes and `[hidden]`, and the contract tests stay green with `liquid-glass` adopted as `pr-theme`.

- [ ] **T15 — The layer guard in `pr-base`** (R15, R16; plan §1.6, C5; after WASM-004 T18)
  - Files: `crates/cobolt-web-player/src/base_css.rs`, `view/contract.rs` (`pr-form-layer` added to the pinned names — `.pr-layer`, the popup furniture at `base_css.rs:201`, is not reused).
  - Do: `position`, `inset`, `z-index`, `isolation`, `overflow`, `pointer-events` of `.pr-form-layer` and of the form root declared `!important` in `pr-base` (an important declaration in an earlier layer beats later layers and unlayered ones); `display`/`visibility` guarded the same way for the hidden state.
  - Verify: player `contract` — the guard rules exist in `pr-base` with `!important`, and `pr-form-layer` is on the layer element WASM-004 T18 creates. Chrome (T25's session, **AC10** at run time): a form sheet setting `.pr-form-layer { z-index: 0; pointer-events: none }` leaves the computed values unchanged.

## Files, manifest and Build (R4, R13, R14; Q2, Q3)

- [ ] **T16 — `cobolt.toml`: the project stylesheet and the theme file** (Q3, Q3a)
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`WebSettings`, `MobileSettings`: `stylesheet: String` default `<forms folder>/application.css`, `theme_file: Option<String>`, both `#[serde(default)]`).
  - Verify: compiler `project_kind` — a manifest without the keys loads with the defaults and writes back byte-identical; the defaults point into `webforms/` and `mobileforms/`; a desktop manifest is unchanged.

- [ ] **T17 — The `.css` files are created, moved and deleted with their owners** (R4, R6, R14; plan §1.4)
  - Files: `crates/cobolt-project-tools/src/create.rs` (`create_project_with` `:703`: Web/Mobile write the project stylesheet; the main form's `.css` beside the `.cfrm` written at `:369`), `crates/cobolt-ide/src/app.rs` (`save_new_form_to` `:14759` writes `<Form>.css` from `web_themes::form_stylesheet` on Web/Mobile kinds; `delete_form_path` `:18994` asks before deleting a `.css` whose generated block or developer section was edited, through the existing confirmation host), `crates/cobolt-ide/src/project_fs.rs` (`move_path` `:206` moves a form's sibling `.css` with it, refusing on a collision before moving either).
  - Do: files read and written as bytes (no reformatting, no line-ending change); nothing created for a Desktop project; the Build refuses a form whose stem equals the project stylesheet's stem. No separate form-rename path exists in `cobolt-ide` today (only `project_fs::rename_folder` `:158`, which moves the folder with both files); if WASM-001 adds one, it moves the `.css` too.
  - Verify: project tools `create` — a new Web project has `webforms/application.css` and `webforms/<main>.css`, a Desktop one has no `.css`; IDE — a new web form gets its `.css`; deleting a form with an untouched sheet deletes both, with an edited one asks first and keeps it on Cancel; `move_path` carries the `.css`; a file with CRLF, comments and odd spacing read and written is byte-identical (**AC4**, round-trip half).

- [ ] **T18 — Theme switch never rewrites developer text** (R4, R6; plan §1.4)
  - Files: the IDE caller of `cobolt_forms::model::Form::apply_theme_defaults` (`model.rs:9463`, where a theme switch re-stamps the controls; the caller is located at `/implement`), `crates/cobolt-ide/src/i18n.rs`.
  - Do: on a Web/Mobile form, the generated block is replaced only when `generated_block_untouched`; otherwise the IDE asks (keep / replace the block / cancel); the developer section is never touched.
  - Verify: IDE — an untouched sheet switches silently and its developer section is byte-identical; a touched one opens the question and is unchanged on Cancel.

- [ ] **T19 — The bundle reads, checks and reports the sheets** (R13, R4, Q5; AC7; plan C10, §1.7)
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`FormInput.stylesheet` `:82`, `BundleInput.stylesheet` `:210` filled verbatim; new `theme.css` and manifest key `theme`; the report), `csp.rs` (`style-src` `:76`, `font-src` `:78` from the sheets' `sources`), `crates/cobolt-project-tools/src/web/mod.rs` (the request carries the sheet paths; `stylesheet_lint` errors stop the Build with file and line).
  - Verify: bundle — sheets copied byte for byte; a sheet with a lint error refuses the bundle naming file and line; the report lists exactly the origins the sheets use under `style-src` and `font-src`, and the CSP the bundle writes allows exactly those (**AC7**); `theme_file` set replaces the built-in `theme.css`.

## IDE half (R2, R5–R7, R11, R12)

- [ ] **T20 — Properties rows `CssClass` and "Style (CSS)"** (R2, R14; Q6)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (a "CSS" group in the control's sections, Web/Mobile kinds only), `i18n.rs`.
  - Do: `CssClass` a single-line text row; `CssStyle` a multi-line row whose `stylesheet_lint` diagnostics (as a declaration list) show under it; written through `InspectorAction.set_props`; an empty value removes the property.
  - Verify: IDE `properties` — on a web form both rows appear and set the props; on a desktop form neither appears and a saved desktop form carries neither key; a bad declaration shows its diagnostic.

- [ ] **T21 — The stylesheet modal** (R5, R6, R7; AC4, AC9)
  - Files: `crates/cobolt-ide/src/panels/stylesheet_modal.rs` (new, on `panels/leaderboard_modal.rs`'s pattern: explicit size, opening size, min/max, `resize_grip` as the only writer), `panels/properties.rs::show_form` (`:11679`: the "Stylesheet…" button on Web/Mobile kinds), `crates/cobolt-ide/src/app.rs` (hosting), `i18n.rs`. Read the `egui-resize-guardian` agent notes and the `egui-paint-regressions` skill first.
  - Do: a code editor with line numbers and CSS colouring from a small Rust tokenizer (comments, strings, at-rules, selectors, properties, values); the diagnostics list under it with lines, a click jumps to the line; **Apply** writes the file (bytes) and pushes the text to an open Preview; **Cancel** asks when the buffer changed; **Revert to theme default** asks, replaces the buffer only (undoable by Ctrl+Z and by Cancel; nothing reaches disk before Apply); "Undo last apply" keeps the previous file text for the session.
  - Verify: IDE `stylesheet_modal` — the button opens it; a deliberately bad sheet lists its diagnostics with lines; Apply writes, Cancel discards, Revert asks then restores the generated text and is undone by Cancel (**AC4**); the tokenizer classifies a sample sheet as expected; **AC9**: the window rectangle is identical over 120 frames in each of the six languages and keeps a dragged size (as `leaderboard_modal.rs::the_size_only_changes_by_a_drag` `:1207`, with a warm `Context`).

- [ ] **T22 — Overridden properties marked; the designer badge** (R12, R16; plan §1.5)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (per-row mark), `crates/cobolt-ide/src/panels/designer.rs` (corner badge), `i18n.rs`.
  - Do: for the selected control, cascade the project and form sheets with `cobolt_forms::css::Stylesheet::parse` + `cascade` (`css.rs:199`) over an `ElementRef` path (form root → control: id, `pr-control`, type class, `CssClass` names) plus `CssStyle`; map each winning declaration back to its property through the table's CSS column; the row shows "overridden by CSS (file:line)", or "may be overridden" for rules under `@media`/`@container` or with pseudo-classes (the cascade evaluates `@media` at `MEDIA_WIDTH_PX` `:64` and ignores layers); an R16 rule is marked the same way. The canvas draws a small badge on a control with `CssStyle` or a matching class rule and does not paint the CSS.
  - Verify: IDE — a form sheet `#BTN-1 { background-color: red }` marks `BackgroundColor` with its file and line; a rule under `@media` gives "may be overridden"; the badge appears for `CssStyle` and for a class rule, not for a type-only rule from the generated `pr-theme` block (which the properties beat).

- [ ] **T23 — Preview updates a style change in place** (R11, AC8; with WASM-008)
  - Files: `crates/cobolt-project-tools/src/web/devserver.rs` (the poll script at `:486` reloads the page today; for a change that touched only `.css` files it calls `player.set_stylesheet` / `set_form_stylesheet` / `set_theme` with the fetched text instead), the change list the watcher exposes (protocol owned by WASM-008).
  - Verify: project tools `devserver` — a `.css`-only change is reported as such and the script takes the in-place branch; a `.wcfrm` change still reloads. Browser (**AC8**): Apply in the modal → the changed computed style in the open Preview, time measured against the **500 ms** budget and written here; `set_form_stylesheet` driven for the first time.

- [ ] **T24 — i18n** (spec §6)
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do: `Tr` fields in six languages (en, es, pt-BR, fr, ja, zh) for the button, the modal's title and buttons, the confirmations (Revert, Cancel with changes, delete an edited sheet, theme switch), every lint code's message (the lint returns codes plus arguments), the "Style (CSS)" and `CssClass` labels, the override marks, the badge tooltip.
  - Verify: IDE `no_empty_ui_translations` (`i18n.rs:15273`) and a new `the_stylesheet_strings_are_in_every_language` that also checks every lint code has a message.

- [ ] **T25 — System KB** (spec §6)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`property_reference_for` `:6380`: `CssClass`, `CssStyle`; `control_structure` `:7797` and the form/project notes: the two stylesheets, the cascade of plan §1.1 with level 6's stated difference from a real inline style, the layer guard), `assets/knowledge/chunked.data` regenerated with `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: compiler `property_reference` for both names; IDE `prebuilt_chunked_kb_matches_the_published_documentation` (`grace_host.rs:4780`) green.

## Browser runs (AC2, AC3, AC5, AC6, AC10)

- [ ] **T26 — Cascade, isolation and the guard in Chrome** (R8, R9, R15, R16; AC5, AC6, AC10)
  - Files: a fixture `.wcfrm` and its sheets under the player's test fixtures (Rust test data, no `.css` committed as source — the sheets are built by the fixture's Rust code).
  - Do: **AC5** — `background-color` set at each of the six levels of plan §1.1; the visible value is the highest level's, and removing levels from the top steps down the table. **AC6** — host page `button { color: red }` and a form sheet `button { … }`: neither crosses the boundary; `--pr-accent` set on the mount element changes the accent. **AC10** — T15's guard holds with the offending rule applied anyway.
  - Verify: the computed values per step written here.

- [ ] **T27 — Exact properties give the expected computed style** (R1; AC2)
  - Do: the gallery bundle; each `Exact` row set to a distinct value through the host API; `getComputedStyle` compared.
  - Verify: property → expected → got written here; zero mismatches among `Exact` rows.

- [ ] **T28 — A new form's stylesheet reproduces its theme** (R4, Q5; AC3)
  - Do: per built-in web theme, a new form with its generated sheet vs the same form with properties only, in the same browser run; differing pixels counted.
  - Verify: the ratio per theme written here, under the plan §5 tolerance (≤ 1 % of pixels differing by more than 8/255 in any channel).

## Desktop unchanged (R14)

- [ ] **T29 — No desktop form changes**
  - Files: tests only (`crates/cobolt-forms`, `crates/cobolt-ide`).
  - Verify: forms — a desktop `.cfrm` fixture loads and saves byte-identical and carries neither `CssClass` nor `CssStyle`; ProgressBar's `Style` still `Continuous`/`Blocks` (`paint.rs` tests at `:25940`, `:26150` green); IDE — a desktop form shows no Stylesheet button and no CSS rows; a desktop project creates no `.css`.

## Finalize

- [ ] **T30 — Finalize** (2.x.x line: no documentation task; `README.md` and `docs/` are never touched)
  - Bump `z` once in `crates/cobolt-ide/src/version.rs`; one `CHANGELOG.md` entry above the previous heading, dated, saying "Written on features-2x".
  - Run the tests of the crates touched, `--no-fail-fast`, reading every `test result` line: `cargo test --no-fail-fast -p cobolt-forms --features render`, `-p cobolt-compiler`, `-p cobolt-web-bundle`, `-p cobolt-project-tools`, `-p cobolt-ide --bin cobolt-ide`, and `cargo test --no-fail-fast --manifest-path crates/cobolt-web-player/Cargo.toml --lib`.
  - Verify: all green; the credential scan of the staged diff is clean.

## Done criteria

Every acceptance criterion is covered by a task's verification: **AC1** T12 · **AC2** T27 · **AC3** T14, T28 · **AC4** T17, T21 · **AC5** T8, T26 · **AC6** T26 · **AC7** T13, T19 · **AC8** T23 · **AC9** T21 · **AC10** T13, T15, T26. Today none is fully met (T1's test covers the theme-owned properties only). Q2a, Q3a, Q4 and Q6 stay open with the working assumptions above.
