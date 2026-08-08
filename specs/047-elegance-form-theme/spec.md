# Spec — Elegance form theme

- **Status:** draft → **ready for review** (all open questions resolved)
- **Folder:** specs/047-elegance-form-theme/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-08

## 1. Overview

Spec 007 gave forms a selectable **theme catalog** — the built-in procedural
`liquid-glass` (default) plus **asset-pack** skins (`stainless-steel`,
`dark-wood`, …). Spec 039 separately brought in the third-party crate
`egui-elegance` (internal dependency name only — never user-facing, per
`specs/steering/product.md`) to draw four real widgets (Knob, Gauge, Switch,
FileDropZone) with their **own** always-on fixed "Slate" look, independent of
whatever form theme is selected.

This spec promotes that crate's visual language into a **second built-in
procedural entry** in the *same* catalog from spec 007: **Elegance**. Selecting
it (project default or per-form override, exactly like any other catalog
theme) re-skins **every** visible control on the form — not just the four
spec-039 widgets — in the Elegance look, in both the designer canvas and the
live/running form. The four spec-039 widgets keep rendering with the crate's
real widgets, but now correctly participate in the chosen theme instead of
silently defaulting to Slate regardless of what the developer picked.

"Elegance" is the only user-facing name for this theme. The crate name
`egui-elegance` and its Rust type names (`Theme`, `Palette`, `Accent`, …) are
implementation details and must never appear in UI strings, docs, or generated
COBOL — consistent with the existing rule that the `cobolt-*` crate prefix
never leaks either.

## 2. Goals / Non-goals

### Goals
- Add **`elegance`** as a new `FormTheme` (`ThemeKind::Procedural`, alongside
  `liquid-glass`) in `cobolt_forms::theme::ThemeCatalog`, selectable the same
  way as any catalog entry (project default in `cobolt.toml`, per-form
  override in `.cfrm`) — reusing spec 007's selection/resolution/persistence
  machinery unchanged.
- Skin **every visible control family** the form renderer supports (the same
  coverage spec 007's R7 established for asset packs: containers, button,
  text input, checkbox/radio, list/combo, slider, progress bar, group box,
  tabs, menu/tool/status bars, tree view, data grid, charts, and the spec-039
  widgets) in both:
  - the **designer canvas** (`paint::draw_control`'s hand-painted static
    face — no live `Ui`, same constraint the spec-039 Knob/Gauge proxies
    already work under), and
  - the **live/running form** (`render::render_interactive`) — using **real
    `egui-elegance` widgets** wherever the crate has a matching one (Button,
    TextInput, TextArea, Checkbox, Select, Slider, ProgressBar, Card,
    TabBar, MenuBar, …), and a theme-consistent procedural paint for any
    control the crate has no widget for.
- When Elegance is the active theme in an interactive render, install the
  real `elegance::Theme` (Slate palette) on the `egui::Context` so the
  existing Knob/Gauge/Switch/FileDropZone widgets pick up the same palette
  as everything else, instead of the crate's un-installed default fallback.
- Fall back cleanly (to Liquid Glass, per-control) for any control kind not
  yet mapped, exactly like spec 007's asset-pack fallback rule (its R11) —
  Elegance never fails to render a form, it just leaves gaps until covered.
- A control's explicit `ForegroundColor`/`BackgroundColor` still wins over
  the theme's defaults, same as spec 007's R12.

### Non-goals
- No new control **types** or behavior changes — visual only, same boundary
  spec 007 drew.
- No asset files — Elegance is 100% procedural (code-drawn), like Liquid
  Glass; it does not use the `theme_pack.rs` 9-slice engine at all.
- No light-mode / multi-palette variant selection. `egui-elegance` ships
  four palettes (Slate, Charcoal, Frost, Paper); this spec ships **one**
  catalog entry ("Elegance") using **Slate** — the crate's own default and
  the palette already hand-approximated in today's designer-canvas proxy.
  Exposing the other three as additional catalog entries/variants is a
  possible future spec, explicitly **not** this one (Q1, resolved).
- No themed background (spec 007's R8/`use_theme_background`) for Elegance
  in v1 — it is a control-chrome theme only, same starting scope spec 007's
  reference pack shipped with before backgrounds were added.
- Desktop-binary / WASM embedding of Elegance rides on spec 007's own T10
  (blocked on spec 006) and spec 006's shared render loop — not re-solved
  here; Elegance uses the same `draw_control`/`render_interactive` entry
  points so it inherits that parity for free once T10 lands.

## 3. User stories
- As a developer, I want to pick **Elegance** as my project's or a form's
  theme, the same way I'd pick any other theme, and have every control on
  that form — not just the Knob/Gauge/Switch/FileDropZone widgets — take on
  a consistent, modern look.
- As a developer who already placed a Knob or Switch on an Elegance-themed
  form, I want it to visually match the rest of the form instead of looking
  like a different, unthemed product bolted on.
- As a developer, I want my existing forms (no theme, or `liquid-glass`, or
  an asset-pack theme) to render **exactly as they do today** — Elegance is
  purely an additional, opt-in catalog choice.

## 4. Requirements (EARS)

**Catalog & selection (reuse of spec 007)**
- **R1 (ubiquitous):** The catalog shall contain a built-in procedural entry
  with id `elegance` and display name "Elegance", alongside `liquid-glass`.
- **R2 (ubiquitous):** Elegance shall be selectable exactly like any other
  catalog entry — project default (`cobolt.toml`) with per-form override
  (`.cfrm`), resolved by the existing `form ?? project ?? liquid-glass`
  precedence — with no changes to `resolve_theme_id`/`ThemeCatalog`'s public
  shape beyond adding the entry.
- **R3 (event):** Selecting Elegance (project default or per-form override)
  shall immediately re-render the affected form(s) in the designer (WYSIWYG),
  same as spec 007's R4.

**Rendering coverage**
- **R4 (ubiquitous):** Elegance shall skin, in both the designer canvas and
  the live/running form, at minimum: Panel/GroupBox (containers), Button,
  TextBox, Label, CheckBox, RadioButton, ListBox, ComboBox, Slider,
  ProgressBar, TabControl, MenuBar/ToolBar/StatusBar, TreeView, DataGrid,
  and the six chart types — plus the existing Knob, Gauge, Switch,
  FileDropZone.
- **R5 (constraint):** *(Amended at `/plan` — see Q5.)* Every Elegance control
  face shall be **hand-painted from the crate's public Slate palette**, on
  **both** surfaces, so that a control renders at the exact geometry the
  developer gave it and the designer canvas and running form are painted by
  the same code. Real `egui-elegance` widgets shall **not** be substituted
  for control faces, because they cannot be constrained to a caller-supplied
  rect. The **only** exceptions are the four spec-039 controls (Knob, Gauge,
  Switch, FileDropZone), which already render as real crate widgets and
  shall continue to (see R6). This covers the kinds the crate has no widget
  for — **DataGrid, TreeView and the six chart types** — by the same rule;
  they are in R4's coverage, not fallback cases (Q4, resolved).
- **R6 (event):** When Elegance is the active theme for an interactive
  render, the real `elegance::Theme` (Slate palette) shall be installed on
  the `egui::Context` before the control-draw pass, so Knob/Gauge/Switch/
  FileDropZone and every other Elegance-skinned control share one consistent
  palette.
- **R7 (state):** While a control kind has no Elegance mapping yet (neither
  a real widget nor a hand-painted face), the renderer shall fall back to
  Liquid Glass for that control rather than fail to render.
- **R8 (constraint):** A control's explicit `ForegroundColor`/
  `BackgroundColor` shall still apply on top of the Elegance defaults,
  same as spec 007's R12.

**Naming & steering**
- **R9 (constraint):** No user-facing surface (theme picker, docs, generated
  COBOL, tooltips) shall show the strings `egui-elegance`, `elegance` (crate
  casing), or any Rust type name from the crate — only "Elegance".
- **R10 (constraint):** Existing forms with no theme set, or with
  `liquid-glass`/an asset-pack theme selected, shall render **exactly as
  today** — Elegance is additive only.
- **R11 (constraint):** The feature shall not be considered complete until
  **every** control family in R4 is covered on **both** surfaces; there is
  no partial-coverage ship point (Q3, resolved). R7's Liquid Glass fallback
  is a runtime safety net, not a permitted delivery state.
- **R12 (state):** While Elegance is the active theme, the form's
  `GlassStyle` (Classic/Enhanced/Neumorphic/NeumorphicDark) shall have no
  effect — Elegance is a top-level catalog theme, not a Liquid Glass
  variant. It shall apply only where R7's Liquid Glass fallback is in play,
  since that fallback *is* Liquid Glass.
  **Sub-element caveat (verified, for `/plan`):** in `draw_control` today the
  theme-vs-glass choice is an if/else-if chain — frameless → asset-pack skin
  → glass — so only the **frame** honours that exclusion. Sub-element paints
  (the CheckBox tick box and the other unconditional `draw_glass_auto` calls
  after the frame dispatch) still take the glass style even under an
  asset-pack theme; this is the gap spec 007's Phase 6 (T15–T17,
  sprite/composite controls) was written to close and is **not** fixed for
  asset packs by this spec. Elegance must therefore paint its own
  sub-elements rather than inheriting the frame-level bypass.
- **R13 (constraint):** The sub-element call sites in R12 shall be routed
  through a **single shared dispatch seam** rather than per-site
  theme conditionals, so they are restructured once. This spec shall
  implement **only the Elegance arm** of that seam: the Liquid Glass and
  asset-pack arms shall pass through to the existing `draw_glass_auto`
  behaviour **unchanged and pixel-identical** (R10/AC8). Filling in the
  asset-pack arm remains spec 007's Phase 6 (T15–T17) and is explicitly
  **not** in scope here — the seam only gives that future work a defined
  home instead of a second round of call-site surgery.

## 5. Acceptance criteria
- [ ] **AC1** — "Elegance" appears in the project-default and per-form theme
  pickers (alongside Liquid Glass and any discovered asset packs) with no
  picker code changes beyond the catalog gaining the entry (R1, R2).
- [ ] **AC2** — Selecting Elegance re-renders the open designer form
  immediately in the new look (R3).
- [ ] **AC3** — Every control kind listed in R4 renders with an
  Elegance-consistent face in the designer canvas AND the running form;
  visual check against the crate's own Slate palette / real widget preview
  (R4, R5).
- [ ] **AC4** — A form containing a Knob/Gauge/Switch/FileDropZone under
  Elegance shows those widgets sharing the same palette as the rest of the
  form (not the crate's un-installed default) (R6).
- [x] **AC5** — Every control family in R4 is covered on both surfaces (no
  family left on the Liquid Glass fallback at delivery); the fallback path
  itself is proven by a test using a synthetic unmapped kind, so it degrades
  gracefully rather than failing (R7, R11).
- [x] **AC6** — Setting a control's own `BackgroundColor`/`ForegroundColor`
  under Elegance still shows that color, not the theme default (R8).
- [x] **AC7** — `grep -ri "egui-elegance"` and `grep -rn "elegance::"` over
  `docs/`, i18n strings, and generated-COBOL templates return nothing (R9).
- [ ] **AC8** — A form with no theme / `liquid-glass` / an existing asset
  pack renders pixel-identically to before this change (R10) — regression.
- [x] **AC9** — Under Elegance, changing the form's `GlassStyle` produces
  **no** visual change on any Elegance-covered control, frame *and*
  sub-elements (e.g. a CheckBox's tick box does not turn neumorphic) (R12).
- [x] **AC10** — With the shared seam in place, a Liquid Glass form and an
  asset-pack-themed form render pixel-identically to before the seam was
  introduced, across all four `GlassStyle` values — proving the pass-through
  arms changed nothing (R13, and the mechanism behind AC8).

## 6. Constraints & steering check
- **i18n (6 languages):** The display name "Elegance" is a **product term,
  not localised** — same treatment as "Liquid Glass" and the asset-pack
  names (`FormTheme::display_name` doc comment). No new `Tr` keys expected
  unless `/plan` finds a picker string that isn't already generic.
- **Generated-code / regenerate contract:** Unaffected — theming is a
  rendering concern; the theme id persists in `cobolt.toml`/`.cfrm` exactly
  like any other catalog entry (spec 007's existing schema, no new fields
  needed beyond the catalog gaining an entry).
- **Docs (English guide):** `docs/developers-guide-en.md`'s "Form themes"
  section (added by spec 007) must gain an Elegance entry — same update
  obligation as any other user-observable feature (CLAUDE.md rule #3).
- **Fix vs feature:** **Feature** (new capability beyond current IDE scope)
  → minor version bump + CHANGELOG at finalize, and an f=96 forum
  announcement per CLAUDE.md rule #4b (never mixed with any concurrent fix
  commit, per rule #5).
- **Product naming:** "Elegance" only, everywhere user-facing (R9); the
  `egui-elegance` crate name stays build-only, same rule that already
  applies to `cobolt-*` crate names.
- **Relationship to spec 007 / 039:** This spec extends spec 007's catalog
  and reuses its selection/persistence/fallback machinery verbatim; it
  completes spec 039's "designer canvas is a simplified stand-in" note by
  giving the crate's look an actual selectable home instead of being
  hard-coded to 4 widget types.

## 7. Open questions

- **Q1 (palette variants) — RESOLVED:** v1 ships **Slate only**, as a single
  catalog entry named "Elegance". Frost/Charcoal/Paper (and any light-mode
  option) are **out of scope** — a possible future spec, not this one.
  Recorded as a non-goal in §2.
- **Q2 (mutual exclusion with the Glass style knobs) — RESOLVED:**
  `GlassStyle` (Classic/Enhanced/Neumorphic/NeumorphicDark) **does not apply
  while Elegance is active**, exactly as it does not apply under an
  asset-pack theme. Confirmed against the code and recorded as R12 — see the
  sub-element caveat there, which is the part `/plan` must actually design
  for.
- **Q3 (coverage order / phasing) — RESOLVED:** **One pass, full coverage.**
  `/tasks` must cover every control family in R4 (both surfaces) before the
  feature is done; no partial-coverage milestone. `/plan` may still order
  the work internally, but there is no "core controls only" ship point.
  Recorded as R11.
- **Q5 (real widgets vs hand-painting) — RESOLVED at `/plan`:** R5 originally
  required real `egui-elegance` widgets wherever the crate had one. Grounding
  that in the crate source showed it is incompatible with the RAD geometry
  model: `elegance::Button` has **no height override**, `Checkbox`/`TabBar`/
  `Switch` are fully intrinsic, and `ui.put(rect, w)` centres a widget at its
  intrinsic size rather than stretching it — so a control sized 200×40 by the
  developer would not render at 200×40. The designer canvas also has only a
  `Painter` (no `Ui`), so it must hand-paint regardless; using real widgets
  live would make the two surfaces disagree, breaking spec 007's parity
  guarantee (its R5/AC4). **Resolution: hand-paint every face from the
  crate's public `Palette` (all fields `pub`, no `Context` needed), except
  the four spec-039 widgets which stay real.** R5 amended accordingly.
- **Q4 (DataGrid / TreeView / chart depth) — RESOLVED:** `egui-elegance` has
  no grid/tree/chart widget, so these get **hand-painted Elegance-palette
  procedural faces on both surfaces** — the same treatment charts already
  get under spec 007's asset-pack palette hook. They are **in** R4's
  coverage, not a fallback cut. Recorded in R5.
