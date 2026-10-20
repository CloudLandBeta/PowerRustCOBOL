<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — CSS3 for appearance, and the form's default stylesheet

- **Status:** draft (needs `/clarify`; Q1 is the rule that decides what the developer sees)
- **Folder:** specs/WASM-007-css3-styling-and-form-stylesheet/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator items **7** (the current properties that take care of appearance can be edited by hand or through CSS3) and **8** (the form gets an editable default CSS, in a modal window
  opened by a button in the Properties pane). They are one subject — *where the look of a web form lives* — so they are one spec.
- **Depends on:** WASM-002, WASM-004. **Needed by:** WASM-006 (link states), WASM-008 (live preview), WASM-009 (the Authentication control's look).

## 1. Overview

On the desktop a control's look is a set of **properties** (BackgroundColor, the gradient, the shadow, the font, the border…) edited in the Properties panel and painted by the egui engine. On the web the
look is **CSS**. This spec says how the two meet: every appearance property is turned into CSS by one table; a developer who prefers CSS can write it, per control (a class and an inline style) and per form
(a **default stylesheet** with its own editor); and the rule that decides which wins is stated once.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **Appearance properties.** Universal seeds (`crates/cobolt-forms/src/model.rs:5240-5300`): BackgroundColor, BackgroundGradient{Enabled, StartColor, EndColor, Direction}, ForegroundColor, FontName, FontSize,
  Bold, Italic, Underline, Strikethrough, Tooltip, Cursor, Padding (+ Left, Top, Right, Bottom), Transparency 0–100 (a legacy `Opacity` is migrated, `:4949`), Shadow{Enabled, Opacity, Color, LightColor, Direction,
  Distance, Blur, BlurStrength}. Per type (`:5347-5571`): CornerRadius, BorderStyle, BorderColor, BorderWidth; BorderGradient*/BorderBlur/BorderTransparency on charts only; a per-corner radius on the Snackbar
  only (`:6485`). **Absent:** LineHeight, LetterSpacing, FontWeight. Values are `PropValue::{String, Int, Bool}` (`:54`); colours are `#RRGGBB[AA]` strings.
- **How they are edited:** `panels/properties.rs` — an Appearance section (`:5167`), a Shadow section (`:5497`), colour rows with a picker (`color_row` `:13872`, picker `:388-454`); edits flow out as
  `InspectorAction.set_props`.
- **CSS-like things that exist:** `cobolt-forms/src/css.rs` is a real cascade, but **only for the Viewer's HTML**. `ThemeDefaults` (`model.rs:4659`) is a base plus per-control-type overrides over
  `THEME_OWNED_PROPS` (`:4623`), stored per project as "<theme>/<glass>", edited in `theme_defaults_modal.rs` and **stamped onto controls when the theme is switched** (`:9463`) — it is not a live cascade.
  `style.rs` holds the "modern" `control_style`. Breakpoint overrides are sparse (control, property, value) rules (`layout/breakpoints.rs:19-35`). **There is no class or stylesheet concept for controls.**
- **The form (`model.rs:8804-8990`):** background_color, four gradient fields, transparency (u8), background_image + `bg_image_mode`, `theme: Option<String>`, `use_theme_background`, `glass_style` (Classic, Enhanced,
  Neumorphic, NeumorphicDark), `control_style`, `responsive`, `layout`, `breakpoints`. A theme resolves form → project `[forms] theme` → `liquid-glass` (`theme.rs:218`); the catalogue has `liquid-glass`,
  `elegance` and `spatial` plus asset packs (`theme.rs:121`). **There is no per-form stylesheet field.**

## 2. Goals / Non-goals

- **Goals:**
  - One table that maps every appearance property to CSS3, used by the runtime and by the checks, with each entry marked *exact*, *approximate* or *not on the web*.
  - Per-control `CssClass` and `Style`, and a per-form **default stylesheet** with an editor in a modal window.
  - A stated precedence between theme, stylesheet, class, property and inline style.
  - Live preview of a style change (WASM-008) without a rebuild.
  - A stylesheet that cannot reach out of the form, and cannot load what the application did not ship.
- **Non-goals:**
  - A CSS preprocessor, utility-class framework or visual CSS designer.
  - Rendering arbitrary CSS in the egui Form Designer (it shows the properties' look; the webview shows the CSS, WASM-008).
  - Changing the desktop style system, `ThemeDefaults` or the themes' look.

## 3. User stories

- As a developer who knows CSS, I want to style my web form with a stylesheet, so that I use the tool I already know.
- As a developer who prefers the Properties panel, I want it to keep working, so that CSS is optional.
- As a developer, I want to see the CSS that my current theme produces, so that I can start from it and change only what I need.
- As a web developer embedding the application, I want my page's colours to flow in through CSS custom properties, so that it matches my site.

## 4. Requirements (EARS)

**Properties ⇄ CSS**
- **R1 (ubiquitous):** Each existing appearance property shall map to CSS3 declarations in the web output through **one table** (for example Shadow* → `box-shadow`, the gradient → `linear-gradient`, Transparency →
  `opacity` or an alpha channel, Padding* → `padding`, CornerRadius → `border-radius`, BorderStyle/Color/Width → `border`, the font properties → `font-*`/`text-decoration`). Each entry shall be marked *exact*,
  *approximate* or *not on the web* (for example perspective `RotateX`/`RotateY` and the operating-system blur are candidates for "approximate"), and the marking shall appear in the property's tooltip (WASM-004 R6).
- **R2 (ubiquitous):** Every control on a web or mobile form shall have **`CssClass`** (space-separated class names) and **`Style`** (inline CSS declarations) properties, in the model and in the Properties panel,
  and its name shall be the element's `id`/`data-name` so that a selector can address it.
- **R3 (ubiquitous):** The element of a control shall carry a class for its type (for example `pr-button`) and the state classes the browser provides through pseudo-classes (`:hover`, `:focus-visible`,
  `:disabled`, `:checked`) shall be usable in selectors.

**The form's default stylesheet**
- **R4 (ubiquitous):** A web or mobile form shall own a **default stylesheet**, stored with the form and round-tripped byte for byte (comments and formatting included). When a form is created the stylesheet shall be
  **generated from its theme**, so that the developer sees the CSS equivalent of the current look.
- **R5 (event):** When the developer presses the stylesheet button in the form's Properties pane, the IDE shall open a **modal window** with a text editor for the stylesheet, CSS syntax colouring, diagnostics (a
  syntax error, an unknown property, a value that does not parse, a feature the target does not support, shown as warnings) and the buttons **Apply**, **Cancel** and **Revert to theme default**.
- **R6 (constraint):** **Revert to theme default** shall ask for confirmation and be undoable; the IDE shall never discard stylesheet text the developer wrote without that (the user-code-is-sacred rule).
- **R7 (constraint):** The modal shall not resize itself: it owns an explicit size, lays its children out from it and has one grip as the only writer of the size (`panels/leaderboard_modal.rs` is the reference).

**Precedence and scope**
- **R8 (ubiquitous):** The cascade, lowest to highest, shall be stated in one place and implemented as stated (Q1): theme defaults, the form stylesheet, the control's `CssClass` rules, the properties the developer
  **set explicitly** in the Properties panel, then the control's `Style`.
- **R9 (ubiquitous):** The stylesheet shall apply **inside the form's root only**; it shall not affect the host page (WASM-002 R12), and CSS custom properties for the theme's colours (an accent, a surface, a
  text colour, a radius) shall be defined so that a host page can set them.
- **R10 (ubiquitous):** The supported CSS shall be that of current evergreen browsers and the platform web views (WKWebView, Android's WebView), with `@media` and container queries, `@font-face` for fonts from the
  project's `Assets/`, and `env(safe-area-inset-*)` on mobile.

**Tooling**
- **R11 (event):** When a property, `CssClass`, `Style` or the stylesheet changes, the Preview (WASM-008) shall show the change without a rebuild.
- **R12 (state):** While a control has CSS overrides the designer cannot paint (a class rule or a `Style`), the Form Designer shall mark it so, and shall not pretend to show the result.

**Safety**
- **R13 (ubiquitous):** The stylesheet shall not be able to load a remote resource unless the project allows that origin: `url()` shall be limited to the project's assets, data URIs and allowed origins;
  legacy script-in-CSS constructs shall be refused; the build shall print the style and font sources the Content-Security-Policy must allow (WASM-002 R14).
- **R14 (constraint):** A desktop form shall be unchanged: it has no stylesheet, and the new properties exist only on the web and mobile kinds.

## 5. Acceptance criteria

- [ ] **AC1** — A test enumerates every appearance property and checks that the table has an entry for it with a marking; an unmapped property fails the test.
- [ ] **AC2** — For a reference form, setting each exact-marked property to a distinct value produces the expected computed style in a headless browser.
- [ ] **AC3** — A new form's stylesheet reproduces its theme's look: the form rendered with the stylesheet and with the properties alone are pixel-compared within a stated tolerance.
- [ ] **AC4** — The modal opens from the button, shows diagnostics for a deliberately bad stylesheet, applies, cancels and reverts (with the confirmation); text saved and reloaded is byte-identical.
- [ ] **AC5** — The cascade of R8 holds for a fixture that sets the same property at each level (the visible value is the highest level's).
- [ ] **AC6** — A host-page rule does not change the embedded application, and the application's stylesheet does not change the host page; a host-set custom property changes the application's accent.
- [ ] **AC7** — A stylesheet with a remote `url()` is refused unless that origin is allowed; the printed policy lists the sources actually used.
- [ ] **AC8** — A style change appears in the open Preview within the budget the plan sets (measured and reported).
- [ ] **AC9** — The modal keeps the same rectangle over 120 frames in each of the six languages and after dragging the grip it keeps the dragged size (the self-resizing guard).

## 6. Constraints & steering check

- **i18n:** yes — the modal, its buttons, the diagnostics' texts and the tooltips' markings, six languages.
- **Generated-code contract:** the stylesheet is form data (in the `.cfrm`, CDATA — it must never contain `]]>`, which is a diagnostic — or in a file, Q2), not generated COBOL; the web output reads it.
- **Docs:** `docs/` untouched on 2.x.x.
- **System KB:** **required** — `CssClass`, `Style` and the stylesheet are properties/behaviours of controls and forms: update `property_reference_for`/`control_structure` and regenerate `chunked.data`.
- **Self-resizing windows:** R7 and AC9 apply the project's golden rule; read the `egui-resize-guardian` and `egui-paint-regressions` notes before building the modal.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — Precedence.** R8 puts a property the developer set explicitly *above* the stylesheet (a value typed in the Properties panel is the strongest statement of intent) and the inline `Style` above that.
  The alternative reading of the operator's words — "properties can be edited by hand **or** by CSS" — is that CSS wins and properties are only the defaults. Which? Recommended: R8, with `!important` as the
  developer's escape hatch.
- **Q2 — Where the stylesheet lives:** inside the `.cfrm` (one file per form, no extra file to lose, a CDATA caveat) or as `forms/<Form>.css` (an ordinary file any CSS tool can open)? Recommended: a file beside the
  form, referenced from the `.cfrm`.
- **Q3 — A project-wide stylesheet** (an application stylesheet under the forms' ones) is not asked for; add it now or leave it to a later spec? Recommended: leave it, but reserve its place in the cascade.
- **Q4 — Class naming** (`pr-button`) and the list of custom properties are a public contract once shipped; settle them at `/plan` and pin them with a test.
- **Q5 — The visual bar:** how close to the desktop's Spatial/Liquid Glass look must the CSS get — the glass shadows, the perspective and spec 057's drawn corners have no exact CSS twin. Accept "approximate"
  per property (R1), or require parity for the three built-in themes?
