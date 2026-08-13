<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Pluggable, self-contained form themes

- **Status:** draft → **ready for review**
- **Folder:** specs/050-pluggable-form-themes/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-12

## 1. Overview

Spec 007 gave forms a selectable theme catalogue and spec 047 added **Elegance**
as a second procedural entry beside **Liquid Glass**. Both landed, but the
renderer was never told that some themes *replace* Liquid Glass rather than
layering on it. Two consequences, both live today:

- **Liquid Glass configuration still applies to Elegance.** `Form::glass_style`
  (Classic / Enhanced / Neumorphic Light / Neumorphic Dark) is read
  unconditionally — at [`paint.rs:1670`](../../crates/cobolt-forms/src/paint.rs)
  and [`paint.rs:5974`](../../crates/cobolt-forms/src/paint.rs) — with no
  reference to the active theme. Under Elegance, picking a Neumorphic glass style
  silently **suppresses every drop shadow** (`regular_drop_shadow` returns `None`
  on its first condition when neumorphic), switches user borders to the
  asymmetric neumorphic pair, and adds neumorphic relief under gradient faces.
  The Appearance pane offers Theme and Glass style as two independent pickers, so
  "Elegance + Neumorphic Dark" is a reachable, meaningless combination.
  `apply_glass_style_defaults` compounds it by **writing glass-specific values
  into the model** (form background, gradient flags, per-control shadow
  properties), which then survive a theme switch.

- **Themes are not pluggable.** The procedural look rides a closed two-variant
  enum, `SurfaceStyle { LiquidGlass, Elegance }`, smuggled through egui's
  per-frame store as a `u8`
  ([`paint.rs:7884`](../../crates/cobolt-forms/src/paint.rs)), and is consulted
  through an *is-it-Elegance?* predicate, `elegance_active`, at **11 painter
  sites** across `paint.rs` and `render.rs`. Adding a third procedural theme
  therefore means editing: the id constant, `FormTheme::x()`, `builtin()`,
  `procedural_ids()`, the enum, `from_theme_id`, the `u8` decode, a palette
  accessor, and every one of those predicates. Liquid Glass is the *fallback*
  rather than one implementation among several, so nothing about the design says
  where a new theme plugs in.

This spec makes theme ownership an explicit, declared fact and turns the
procedural look into a registered implementation, so a theme that owns its whole
appearance gets no Liquid Glass configuration applied, and a new theme is one
file plus one registration line.

## 2. Goals / Non-goals

### Goals

- Let a catalogue entry declare that it is **self-contained** — it paints the
  whole look and Liquid Glass configuration must not be applied to it. Liquid
  Glass is not self-contained; Elegance is; an asset pack declares it in its
  `theme.toml` and defaults to *not* self-contained (a pack skins the controls it
  covers and falls back to glass for the rest — spec 007 R11).
- Replace `SurfaceStyle` and the `elegance_active` predicates with a **registered
  procedural theme** (`Arc<dyn SurfaceTheme>`), carried per frame on the same
  channel shape as `set_active_theme`, so Liquid Glass and Elegance are two
  implementations of one trait and each painter asks the theme instead of asking
  which theme it is.
- Keep the existing selection and persistence untouched: per-form `Form::theme`,
  project `[forms] theme` in `cobolt.toml`, resolution `form ?? project ??
  liquid-glass`.
- Make the per-form picker tell the truth about an **inherited** project default
  (today it passes `None` for the project default, so a form inheriting the
  project's Elegance is displayed as "Liquid Glass").
- Preserve the developer's explicit choices: a self-contained theme supplies
  **defaults only**.
- Leave Liquid Glass byte-identical.

### Non-goals

- No new control types, no behaviour changes, no new visual look — this is
  ownership, routing and picker honesty. Adding an actual third theme is a
  separate change that this one is the prerequisite for.
- No declarative (TOML-only) procedural themes. Registration is a Rust trait
  object (operator decision, Q1); a palette-only file format is possible later
  and explicitly not here.
- Not the IDE's own chrome theme (`cobolt_ide::theme`, `apply_glass_visuals`,
  the 16 editor palettes). That is a different axis with its own selector and is
  untouched.
- No change to asset-pack rendering (9-slice, `theme_pack.rs`) beyond reading one
  new manifest field.
- The Elegance palette itself, its `SurfaceRole` mapping, and its coverage gaps
  stay exactly as spec 047 left them.

## 3. User stories

- As a form developer, I want a theme that replaces Liquid Glass to actually
  replace it, so that my drop shadows do not silently disappear because a glass
  style I cannot see is still in effect.
- As a form developer, I want the Appearance pane to stop offering me settings
  the selected theme ignores, so that I can tell what will affect my form.
- As a form developer, I want a form to show me which theme it will actually
  render with, including one inherited from the project, so that I am not told
  "Liquid Glass" while the form renders as Elegance.
- As a form developer, I want the explicit colours, corner radii and
  transparencies I set to keep winning over the theme, so that a theme is a
  starting point and not a straitjacket.
- As a maintainer, I want adding a theme to be one file and one registration
  line, so that new looks do not require editing a dozen painter call sites.

## 4. Requirements (EARS)

### Declaring ownership

- **R1 (ubiquitous):** A catalogue entry shall carry an explicit
  **self-contained** flag stating whether it paints the whole look itself.
- **R2 (ubiquitous):** `liquid-glass` shall be **not** self-contained;
  `elegance` shall be self-contained.
- **R3 (optional):** Where an asset pack's manifest declares the self-contained
  flag, the catalogue shall honour it; where the manifest omits it, the pack
  shall be **not** self-contained.

### The gate — no Liquid Glass configuration on a self-contained theme

- **R4 (state):** While the resolved theme is self-contained, the renderer shall
  not apply the form's `GlassStyle` in any form — not the Classic/Enhanced frost
  stack, not the neumorphic dual relief, not neumorphic user borders, and not
  the neumorphic suppression of drop shadows.
- **R5 (state):** While the resolved theme is self-contained, a control's own
  drop-shadow properties (`ShadowEnabled` and companions) shall be honoured
  exactly as they are under Liquid Glass with a non-neumorphic style — the shadow
  is the developer's, not the glass stack's.
- **R6 (constraint):** The renderer shall not consult the form's `GlassStyle`
  through more than one gate, so a new painter cannot reintroduce the leak by
  reading it directly.
- **R7 (state):** While the resolved theme is self-contained, the IDE shall not
  write `GlassStyle`-derived values into the model — `apply_glass_style_defaults`
  and its per-control counterpart shall not run.
- **R8 (constraint):** Switching a form from a self-contained theme back to
  Liquid Glass shall restore the glass appearance the form had, without the
  developer having to re-enter anything.

### Developer choices outrank the theme

- **R9 (constraint):** A control's **explicitly set** `BackgroundColor`,
  `ForegroundColor`, `CornerRadius`, `BorderStyle`/`BorderWidth` and
  `Transparency` shall outrank the theme's defaults under every theme,
  self-contained or not. (Extends spec 047 R8 from colours to the full set.)
- **R10 (state):** While a self-contained theme is active and a property is
  **unset**, the value shall come from the theme rather than from a Liquid Glass
  default.

### Pluggability

- **R11 (ubiquitous):** The procedural look shall be represented by a
  **registered implementation** obtained from the catalogue, not by a closed enum
  of theme identities.
- **R12 (ubiquitous):** Liquid Glass shall be one such implementation, on equal
  footing with the others, rather than the value every unrecognised selection
  falls back to by construction.
- **R13 (ubiquitous):** A painter shall ask the active theme what to draw for a
  structural role, and shall not test which theme is active.
- **R14 (constraint):** Adding a further procedural theme shall require no edit
  to any painter call site in `paint.rs` or `render.rs`.
- **R15 (event):** When a rendering surface has not published a theme for the
  frame, the renderer shall behave exactly as it does today with no style
  published — as Liquid Glass.
- **R16 (ubiquitous):** All four rendering surfaces — designer canvas, preview,
  Run Form / the form host, and the compiled binary — shall obtain the theme the
  same way, so a theme cannot be honoured on one surface and ignored on another.

### The picker

- **R17 (state):** While the resolved theme is self-contained, the Appearance
  pane's **Glass style** row shall be shown **disabled**, with a hint that the
  theme owns the look.
- **R18 (constraint):** Disabling that row shall not alter the stored
  `GlassStyle` value.
- **R19 (ubiquitous):** The per-form theme picker shall resolve against the
  **project default**, and shall show a form with no override of its own as
  inheriting that default rather than as Liquid Glass.
- **R20 (constraint):** Every new or changed user-facing string shall be a `Tr`
  field translated in all six languages.

### Non-regression

- **R21 (constraint):** A form whose resolved theme is `liquid-glass` — including
  every existing form with no theme set — shall render pixel-identically to
  before this change, at every `GlassStyle`.
- **R22 (constraint):** No user-facing surface shall name the third-party crate
  behind the Elegance palette (spec 047 R9 continues to hold).

## 5. Acceptance criteria

> **All 14 verified by test in 1.61.37** (see `tasks.md` for which task covers
> which). The one thing tests cannot cover is the *look* of an Elegance form
> after the fix — plan §6's manual checklist, still outstanding for the operator.

- [ ] **AC1 (R1–R3)** — The catalogue reports `liquid-glass` as not
      self-contained and `elegance` as self-contained; a pack manifest with the
      flag set is honoured, and one without it reports not self-contained.
- [ ] **AC2 (R4, R5)** — With Elegance selected, a control with `ShadowEnabled`
      paints its drop shadow under **all four** `GlassStyle` values, and the
      painted result is **identical** across those four. A test asserts both the
      presence of the shadow and the identity of the four renderings.
- [ ] **AC3 (R4)** — With Elegance selected and `GlassStyle` = Neumorphic Light,
      no neumorphic relief and no asymmetric user border are painted.
- [ ] **AC4 (R6)** — A test enumerates the sites that read the form's
      `GlassStyle` for painting and asserts they all pass through the single
      gate; adding an ungated read fails it.
- [ ] **AC5 (R7, R8)** — Selecting Elegance then changing `GlassStyle` leaves the
      form's background colour, gradient flags and per-control shadow properties
      untouched; switching back to Liquid Glass reproduces the form's earlier
      glass appearance.
- [ ] **AC6 (R9, R10)** — Under Elegance, a control with an explicit
      `BackgroundColor`, `CornerRadius` and `Transparency` paints with those
      values; the same control with them unset paints the theme's values.
- [ ] **AC7 (R11–R14)** — A test registers a **throwaway** third procedural theme
      in the catalogue and renders a form with it, with **no** change to any
      painter call site. The test reports the number of painter sites that had to
      change: expected **0**.
- [ ] **AC8 (R15)** — A context that never publishes a theme renders identically
      to one that publishes Liquid Glass.
- [ ] **AC9 (R16)** — A parity test renders the same form through the designer
      canvas, the preview path and the host path under Elegance and reports the
      three results as matching on the themed properties under test.
- [ ] **AC10 (R17, R18)** — With a self-contained theme resolved, the Glass style
      row is disabled and carries its hint; toggling the theme back and forth
      leaves the stored `GlassStyle` string byte-identical.
- [ ] **AC11 (R19)** — A form with no theme override, in a project whose default
      is Elegance, displays Elegance (marked as inherited) in the per-form
      picker, and renders as Elegance.
- [ ] **AC12 (R20)** — Every added `Tr` field is present and non-empty in all six
      languages; the existing i18n completeness test covers it.
- [ ] **AC13 (R21)** — The spec-047 `elegance_baseline_*` suite still passes
      unchanged, and a Liquid Glass rendering of a fixture form is compared
      before/after across all four glass styles and reported as identical.
- [ ] **AC14 (R22)** — The catalogue's display names and every added UI string
      are asserted free of the crate name.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — the Glass style row gains a disabled-state hint
  (R17) and the per-form picker gains an "inherited from project" affordance
  (R19). Both are new `Tr` fields in all six languages. No hard-coded literals.
- **Generated-code / regenerate contract:** No impact. Themes are a render-time
  concern; no generated COBOL changes, and no COBOL identifier or source text is
  touched.
- **Docs (English guide):** Yes — `docs/developers-guide-en.md` must gain a
  short passage in the form-theme section explaining that some themes own the
  whole look, that Glass style does not apply to them, and that explicit control
  properties still win. Translations are user-maintained and must not be edited.
- **System KB:** Yes — this changes an observable property behaviour (`GlassStyle`
  becomes conditional on the theme), so the `cobolt-compiler` documentation
  constants must be updated and `cargo run -p cobolt-ide --example
  build_chunked_kb` re-run in the same change, with the regenerated
  `assets/knowledge/chunked.data` committed.
- **Versioning:** bump `z` in `crates/cobolt-ide/src/version.rs` and add a
  `CHANGELOG.md` entry. Only the operator raises `x` or `y`.
- **Fix vs feature classification — this spec is a FIX in its entirety
  (operator ruling, 2026-08-12).**
  - Specs 007 and 047 already specified this behaviour; the implementation did
    not deliver it. That is **technical debt**, treated exactly like a bug fix —
    the same principle CLAUDE.md rule #4 applies to a missing COBOL-85 construct.
  - The code itself records the intent that was never honoured:
    `draw_elegance_surface` is documented as *"no frost, no relief, and **no
    dependence on `GlassStyle`** (spec 047 R12)"*, while `is_neumorphic` is read
    unconditionally two call frames above it.
  - Consequences: every requirement here lands in **fix** commits on the fixes
    branch; announced on **forum f=97 only**; **no f=96 post**.
  - *(Superseded: this section previously proposed splitting R1–R3/R11–R16 out as
    a feature for f=96. That split is withdrawn.)*
- **Hard constraint (operator, standing):** Rust only. No Python, shell, `sed`,
  `perl` or Node used to edit or generate repository files, not even as a
  scratchpad helper.

## 7. Open questions

- **Q1 — How is a theme registered?** **RESOLVED (operator, 2026-08-12):** a Rust
  trait object registered in the catalogue. Compiled in, type-safe, and able to
  carry custom painters — which Elegance already needs. A declarative
  palette-only format is a possible later spec, not this one.
- **Q2 — Under a self-contained theme, who owns `Transparency` and
  `CornerRadius`?** **RESOLVED (operator, 2026-08-12):** the developer's explicit
  values win; the theme supplies defaults only. Recorded as R9/R10, extending
  spec 047 R8 from colours to the full property set.
- **Q3 — What happens to the Glass style picker under a self-contained theme?**
  **RESOLVED (operator, 2026-08-12):** shown but **disabled**, with a hint, and
  the stored value preserved so switching back to Liquid Glass restores it.
  Recorded as R17/R18.
- **Q4 — Commit ordering.** **DISSOLVED by the operator's fix ruling
  (2026-08-12).** The question only existed because golden rule #5 forbade
  putting a fix and a feature in one commit. With the whole spec classified as a
  fix, the better order is simply available: the registry lands first and the
  gate becomes a handful of one-line changes on top of it, written once. See
  `plan.md` §0 and §4 decision 3.
- **Q5 — Should `SurfaceRole` grow beyond its five variants?** **RESOLVED in
  `plan.md` §4 decision 2: no.** Of the 11 predicate sites, only 3 ask for a
  structural face; the other 8 ask for a *named colour to default an unset
  property to*. Those get a second, orthogonal `ColorToken` accessor rather than
  eight new roles, so `Card | Input | Button | Accent | Shape` stands as spec 047
  defined it.
- **Q6 — Does the compiled binary need the flag at build time?** **RESOLVED in
  `plan.md` §1 Movement B: no extra work.** The compiled binary renders through
  the same `cobolt_form_host::FormHost` as Run Form
  ([`host.rs:1072`](../../crates/cobolt-form-host/src/host.rs)), so once the host
  carries the theme instead of the enum the fourth surface is covered. The only
  compiler-side change is `resolve_surface_style` → `resolve_surface_theme`
  ([`lib.rs:1666`](../../crates/cobolt-compiler/src/lib.rs)).

---

**Next step:** review this spec alongside `plan.md`. When satisfied, run
**`/tasks`**.
