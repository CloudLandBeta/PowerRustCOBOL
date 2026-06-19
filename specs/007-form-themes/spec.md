# Spec — Form themes

- **Status:** draft → approved
- **Folder:** specs/007-form-themes/
- **Author:** Anthropic Code Agent   **Date:** 2026-06-18

## 1. Overview

Give the developer's **forms** a selectable, **extensible catalog of visual
themes**, applied by the **shared form renderer** so a themed form looks identical
in the **designer preview, the desktop build, and the WASM/web build** (i.e.
common to every project type). A theme comes in two kinds: the **built-in
procedural** `liquid-glass` (the current look — the default, unchanged) and
**asset-pack "special" themes** — photoreal material skins (initial set:
**stainless steel, dark wood, modeling clay, knitted wool**), each **pixel-
faithful** to a reference design. A theme is chosen as a **project default** with a
**per-form override**, skins the **controls** (and, optionally, a **themed
background**), and the catalog is built to **grow over time** by dropping in new
packs. Existing forms/projects keep the Liquid Glass look with no change.

## 2. Goals / Non-goals

### Goals
- An **extensible theme catalog** (registry) with two theme kinds under one
  selector: built-in **procedural** (Liquid Glass, default) and **asset-pack**.
- Ship the four initial **special packs** (steel, wood, clay, wool), **visually
  faithful** to the provided reference mockups.
- Apply the theme via the **shared renderer** → identical look in designer,
  desktop, and WASM (all project types).
- **Project default theme** (manifest) + **per-form override**; default
  `liquid-glass`.
- Theme **skins controls** and their interactive states; **optional themed
  background** the form can opt into.
- **Self-describing asset-pack format** so a new theme is a **drop-in** (pack +
  catalog entry), no renderer changes.
- **Liquid Glass unchanged**; existing forms render exactly as today.

### Non-goals (explicitly out of scope)
- New control **types** or layout/behaviour changes — theming is **visual only**.
- A **theme-authoring UI** in the IDE (packs are authored as assets; a future
  feature).
- **Animated/video** themes.
- Changing the **Liquid Glass** appearance.
- Bespoke per-control skin overrides beyond the theme (a control may still set its
  own colours — see R12 — but not its own skin pack).
- The exact asset format / pipeline is **/plan** (this spec fixes the *what*).

## 3. User stories
- As a developer, I want to pick a **theme** for my app's forms so it gets a
  distinctive look without styling every control.
- As a developer, I want a **project-wide default** theme and the option to
  **override** it on a specific form.
- As a developer, I want the theme to look the **same** in the designer, the
  desktop build, and the web build.
- As a developer, I want my **existing forms unchanged** (Liquid Glass).
- As the product, I want to **add themes over time** by dropping in a pack.

## 4. Requirements (EARS)

**Catalog & model**
- **R1 (ubiquitous):** The system shall provide an **extensible theme catalog**
  containing at least `liquid-glass` (built-in procedural, **default**) and the
  special asset-pack themes `stainless-steel`, `dark-wood`, `modeling-clay`,
  `knitted-wool`.
- **R2 (ubiquitous):** Each theme shall be one of two **kinds** — built-in
  procedural or asset-pack — exposed through **one selector**. Adding a new
  asset-pack theme shall be a **drop-in** (a self-describing pack + a catalog
  entry) requiring **no renderer code change** and surfacing automatically.

**Selection**
- **R3 (state):** The project shall carry a **default theme** (in the manifest),
  defaulting to `liquid-glass`; an individual **form may override** it; a form
  with no override uses the project default.
- **R4 (event):** When the developer selects a theme (project default or per-form
  override), the **designer shall immediately re-render** the affected form(s) in
  that theme (WYSIWYG).

**Rendering & parity**
- **R5 (ubiquitous):** The selected theme shall be applied by the **shared form
  renderer**, so the form looks **identical** in the designer preview, the desktop
  build, and the WASM/web build, across all project types.
- **R6 (constraint):** Each special theme shall be **visually faithful** to its
  reference design — material surfaces, bevels/frames, corner details (rivets/
  stitches/studs/grain), knobs, button plates, content wells, and **per-state**
  appearance (normal / hover / pressed / disabled / focused).

**Coverage**
- **R7 (ubiquitous):** A theme shall skin **all controls** (panels/containers,
  button, slider, checkbox, radio, text input, label, list/combo, grid, …) and
  their interactive states. This **includes the chart controls** (PIE/LINE/BAR):
  not only the frame/well but the **data marks** themselves (pie slices, line
  strokes/points, bars) take on the theme's palette and material treatment.
- **R8 (optional):** Where a theme provides a **themed background**, the form may
  **opt into** it; otherwise the form's existing **Back color / Background Image**
  applies.

**Defaults & compatibility**
- **R9 (constraint):** `liquid-glass` shall remain the **default and unchanged**;
  a form/project with no theme set shall render **exactly as today**.

**Asset-pack format**
- **R10 (ubiquitous):** An asset-pack theme shall be defined by a **self-
  describing pack** — per-control / per-state imagery (e.g. 9-slice or atlas) with
  **slice metrics**, an optional **background**, and a **palette/typography spec**
  — that the catalog **discovers** without code changes.
- **R11 (state):** While a control's theme does not provide imagery for that
  control, the renderer shall **fall back** (to Liquid Glass for that control, or
  a generic themed panel — decided in /plan) rather than fail.

**Cross-cutting (steering & portability)**
- **R12 (constraint):** A control's explicit colour properties
  (`ForegroundColor` / `BackgroundColor`) shall still apply **on top of** the
  theme (theme provides defaults; explicit per-control colours win).
- **R13 (constraint):** The theme system and the special packs shall be
  **portable to WASM** (no native-only assets/APIs; assets ship in the bundle), so
  themed forms render in the browser (consistent with spec 006).
- **R14 (constraint):** All new user-facing IDE strings (theme picker, per-form
  override, themed-background toggle) shall be `Tr` fields in **all six** languages.
- **R15 (constraint):** The English `docs/developers-guide-en.md` shall document
  form themes (catalog, project default + per-form override, themed background,
  adding packs); translations are user-maintained.
- **R16 (constraint):** Special-theme assets shall be **original work**. They are
  generated by an AI image agent from a **text-only prompt with no references to
  existing artwork** (e.g. "reimagine the Designer window as a cyberpunk
  stainless-steel interface … all panels/buttons/sliders carved from polished dark
  metal with neon accents"), so no third-party material is reproduced — such
  generated-from-scratch assets are treated as original and distributable.

## 5. Acceptance criteria
- [ ] **AC1** — The catalog lists `liquid-glass` + the four special themes;
  selecting each in the designer re-renders the form in that theme (R1, R4).
- [ ] **AC2** — A project default theme is stored in/loaded from the manifest; a
  per-form override is honoured; a form with no override uses the default; a
  project with no theme uses `liquid-glass` (R3).
- [ ] **AC3** — Each special theme renders **faithfully to its reference mockup**
  for the core controls (panel/container, button, slider, label, chart container),
  including per-state appearance (visual check vs the images) (R6).
- [ ] **AC4** — The same themed form looks **identical** in the designer, a
  desktop build, and a WASM build (R5).
- [ ] **AC5** — Enabling a theme's **optional background** shows it; disabled, the
  form's back-color / background-image applies (R8).
- [ ] **AC6** — Existing forms/projects (no theme) render **exactly as before**
  (Liquid Glass) — regression (R9).
- [ ] **AC7** — Adding a new asset-pack theme (drop-in pack + catalog entry) makes
  it appear in the picker and render, with **no renderer code change** (R2, R10).
- [ ] **AC8** — New IDE strings exist in 6 languages (`cargo test -p cobolt-ide
  i18n` green); the English guide documents form themes (R14, R15).

## 6. Constraints & steering check
- **i18n (6 languages):** Yes — theme picker, per-form override, themed-background
  toggle, ×6 (R14).
- **Generated-code / regenerate contract:** Theming is a **rendering** concern; the
  generated `.cbl` and the codegen banner/regenerate contract are **unaffected**.
  The *selection* persists in the model: a **project default** in `cobolt.toml` and
  a **per-form** theme id in the `.cfrm` (additive schema, back-compatible).
- **Docs (English guide):** New "Form themes" section (R15).
- **Fix vs feature:** **Feature** → minor version bump + CHANGELOG at finalize.
- **Portability (spec 006):** assets must work on the `wasm32` target and ship in
  the bundle; keep pack sizes reasonable (flag in /plan).
- **Rendering mechanism:** pixel-fidelity (R6) implies a **texture/asset** approach
  (9-slice/atlas), not pure procedural painting — confirmed by the examples; exact
  format, slice schema, and asset location are **/plan** (see Q1).
- **product.md:** broadens the RAD "look"; note the themable-forms capability.

## 7. Open questions
- **Q1 (pack format & location):** Exact asset format — 9-slice vs texture atlas vs
  per-state full images — the slice-metrics schema, and where packs live
  (committed `assets/themes/<id>/` vs external/downloadable). → /plan.
- **Q2 (per-pack control coverage):** Which controls must each v1 pack cover (the
  mockups show panel/container, button, slider, label, chart container)? Define the
  required set + the **fallback** for uncovered controls (R11).
- **Q3 (selection UI):** Where the theme is chosen — Appearance panel? a dedicated
  Theme picker? — for the **project default** vs the **per-form override**. → /plan.
- **Q4 (fidelity verification):** How "pixel-faithful" is checked — visual diff vs
  the reference screenshots, or manual review? Define the AC3 check.
- **Q5 (charts) — RESOLVED:** **All controls are themed, charts included.** v1
  themes the PIE/LINE/BAR **data marks** (slices/lines/bars) as well as the
  container, using the theme's palette + material treatment (R7).
- **Q6 (asset provenance) — RESOLVED:** The special-theme assets are AI-generated
  from **text-only prompts with no references to existing artwork**, hence 100%
  original and distributable (R16). No further action.
- **Q7 (control colours vs theme):** Confirm explicit `ForegroundColor` /
  `BackgroundColor` override the theme's defaults (R12), and how that interacts
  with image-based control skins.
