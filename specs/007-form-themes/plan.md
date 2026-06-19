# Plan — Form themes

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-18

## 1. Approach

Model a theme as a **resolved style** passed to **one shared control renderer**,
so a themed form looks identical wherever it is drawn (R5). Today rendering is
*almost* shared: `cobolt-forms::paint::draw_control` exists (used by Preview/Run
and intended for the desktop/web binary) but the **designer still calls its own
`panels::designer::draw_control`**, and the extracted port is partial. So step one
is to **unify on `cobolt-forms::paint::draw_control`** (complete the port, route
the designer through it) — which also serves spec 006's web renderer — then add a
**`theme` parameter** that generalises the current `glass: bool`.

A **`FormTheme`/`ThemeCatalog`** (in `cobolt-forms`) enumerates themes of two
kinds (R1, R2): **procedural** `liquid-glass` (the current glass path, kept
identical — R9) and **asset-pack** specials. An asset pack is a **self-describing
folder** (`theme.toml` + 9-slice/atlas images per control & state + optional
background + palette/typography) that the catalog **discovers** (R10), so new
themes are drop-ins (R2, AC7). `draw_control` renders procedural themes as today
and asset-pack themes by **9-slice compositing** the pack art (with a per-`ctx`
texture cache); a control the pack doesn't cover **falls back** to liquid-glass
(R11). Explicit `ForegroundColor`/`BackgroundColor` still win (R12).

Selection is **project default + per-form override** (R3): a default theme id in
`cobolt.toml`, an optional id on the form (`.cfrm`); resolution =
`form.theme ?? project.default ?? "liquid-glass"`. The designer re-renders live on
change (R4). The desktop binary and the WASM build (006) **embed the selected
pack's assets** and use the same themed `draw_control` (R5, R13).

## 2. Affected crates / files
- `crates/cobolt-forms/src/paint.rs` — complete the `draw_control` port; add a
  `theme: &ResolvedTheme` parameter (replacing/extending `glass: bool`);
  asset-pack 9-slice rendering path + per-state selection; texture lookup.
- `crates/cobolt-forms/src/theme.rs` *(new)* — `FormTheme` (procedural |
  asset-pack), `ThemePack` (manifest + slice metrics + palette/typography),
  `ThemeCatalog` (discovers built-in + packs), resolution helper, and a texture
  cache keyed by `(theme, control-part)`.
- `crates/cobolt-forms/src/model.rs` — `Form.theme: Option<String>` (per-form
  override) + `use_theme_background: bool` (R8).
- `crates/cobolt-forms/src/xml.rs` — persist/load the per-form theme + bg flag
  (additive `.cfrm`; absent ⇒ `None`).
- `crates/cobolt-ide/src/project_model.rs` — project **default theme** (a
  `[theme]` table or `RuntimeConfig.theme`), default `liquid-glass`/empty.
- `crates/cobolt-ide/src/panels/designer.rs` — route designer rendering through
  the unified `paint::draw_control`; resolve + pass the theme; the per-form theme
  control in the Appearance/property pane.
- `crates/cobolt-ide/src/panels/settings_form.rs` — project **default theme**
  picker.
- `crates/cobolt-ide/src/app.rs` — theme resolution wiring; live re-render on
  change; pass theme to all `draw_control` call sites (preview, run, inline).
- `crates/cobolt-compiler/src/lib.rs` — embed the resolved theme's assets in the
  desktop binary + the wasm bundle; pass the theme to the generated render loop.
- `crates/cobolt-ide/src/i18n.rs` — `Tr` fields ×6 (theme picker label, per-form
  override, "use theme background", theme names if localised) (R14).
- `assets/themes/<id>/` *(new)* — `liquid-glass` (metadata only; procedural) plus
  `stainless-steel`, `dark-wood`, `modeling-clay`, `knitted-wool` (manifest +
  images). **Original art required (R16) — see Risks.**
- `docs/developers-guide-en.md` — "Form themes" section (R15).
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — feature minor bump.

## 3. Data / model changes
- **`Form.theme: Option<String>`** + **`Form.use_theme_background: bool`** in the
  `.cfrm` (additive; `None`/`false` defaults keep current behaviour, R9).
- **Project default theme** in `cobolt.toml` (`[theme] default = "…"`,
  `#[serde(default)]` → empty ⇒ `liquid-glass`). Old manifests unaffected.
- **Theme-pack manifest** (`assets/themes/<id>/theme.toml`): id, display name,
  kind, per-control image refs + 9-slice insets (L/T/R/B), per-state variants,
  optional background image + tiling, palette + font hints. The schema is the
  pack's public contract (self-describing, R10).
- No `.cbl` / codegen change — theming is a **rendering** concern.

## 4. Key decisions & alternatives
- **One themed renderer (`paint::draw_control`).** — Why: identical look in
  designer/desktop/web (R5) and it dovetails with 006's shared renderer. Rejected:
  theming the designer and the binary separately (drift). *Prerequisite:* finish
  unifying the two `draw_control`s first (currently divergent).
- **Asset packs = 9-slice + `theme.toml`.** — Why: reaches the photoreal fidelity
  the mockups show (R6) and scales to any control size (fixed corners, stretched
  edges/centre). Rejected: procedural per-material painting (can't match the
  references), full per-size renders (unscalable).
- **`liquid-glass` stays a *procedural* theme, unchanged.** — Why: zero regression
  (R9); it's the default and the universal fallback (R11).
- **Project default + per-form override (Q-clarify).** — Why: "common to all
  projects" plus per-form flexibility. Resolution `form ?? project ?? glass`.
- **Charts fully themed, data marks included (Q5).** — The container/well **and**
  the data marks (pie slices, line strokes/points, bars) take the theme's palette +
  material treatment. Approach: the pack supplies a **chart style** (data palette,
  stroke widths, and an optional **material fill** — a texture/sprite for
  bars/slices and a stroke style for lines); the chart renderer reads it instead
  of its hard-coded colours. Rejected: theming only the frame (user wants charts
  themed). This is the **highest-fidelity-risk** item (see Risks).
- **Per-control coverage = core set; fallback to glass (Q2).** — v1 packs cover
  panel/container, button, slider, label, checkbox/radio, text input, list/combo;
  uncovered controls render liquid-glass (R11).
- **Selection UI (Q3):** project default in the project **Settings** form;
  per-form override in the **Appearance** property pane.

## 5. Risks & mitigations
- **Renderer not yet unified** (designer vs extracted `paint::draw_control`, which
  is partial). → Complete the port + route the designer through it **before**
  theming; gate with a **liquid-glass pixel-regression** check so the default look
  is unchanged. Coordinate with spec 006 (same renderer).
- **Art is an asset-supply dependency (R16 — provenance settled).** The special-
  theme assets are **AI-generated from text-only prompts with no references to
  existing artwork**, so they are original and distributable. The remaining
  dependency is purely **production/delivery**: the operator supplies the
  generated per-control / per-state 9-slice assets (and slicing them into the
  pack format). The engineering (engine + format + Liquid Glass + one reference
  pack) does not block on the full set; Phase 4 consumes the operator-provided
  packs as they arrive.
- **9-slice fidelity at odd sizes** (rivets/stitches/studs at corners). → Author
  art at typical control aspect ratios; document min sizes; corners fixed, only
  edges/centre stretch.
- **Themed chart data marks are the hardest fidelity item.** Bars/slices/lines are
  dynamic geometry, so 9-slice doesn't apply directly. → The chart renderer gains
  a **theme chart-style hook** (palette + stroke + optional material **fill
  texture** tiled/clipped into each bar/slice). Photoreal results (clay wedges,
  knitted bars) depend on the per-pack fill textures; start with palette + flat
  material fill, raise fidelity with pack art. Liquid-glass chart drawing stays
  the default/fallback.
- **WASM bundle bloat** from textures. → Optimise PNGs; embed **only the used
  theme(s)**; lazy-decode.
- **egui texture lifecycle** (handles per `ctx`, wasm embed). → A theme-texture
  cache owned per render context; load from embedded bytes.
- **Pixel-fidelity is subjective (Q4).** → Manual review against the reference
  screenshots is the AC3 check; optional screenshot-diff harness if feasible.

## 6. Test strategy
- **`cobolt-forms`:** catalog **discovery** test (built-in + packs found);
  **resolution** test (`form ?? project ?? glass`); `theme.toml` + 9-slice metric
  parsing; **liquid-glass regression** — a known form rendered with the default
  theme is byte/pixel-identical to pre-change (snapshot or painter-op hash).
  Report counts.
- **`cobolt-forms` model/xml:** per-form `theme`/`use_theme_background` round-trip
  in `.cfrm`; absent ⇒ defaults.
- **`cobolt-ide`:** project-default serde (empty ⇒ liquid-glass);
  `cargo test -p cobolt-ide i18n` (×6, no empty).
- **Manual/visual:** in the designer, select each theme → matches its reference
  (AC3); the same themed form in designer / desktop build / WASM build looks
  identical (AC4); toggle the themed background (AC5); an existing form (no theme)
  is unchanged (AC6); drop a new pack into `assets/themes/` → it appears in the
  picker and renders with no code change (AC7).

## 7. Steering compliance
- [ ] i18n: all new UI strings in 6 languages (R14).
- [ ] Generated-code banner + regenerate-on-action **unaffected** (theming is
      rendering; per-form theme persists additively in `.cfrm`).
- [ ] English dev guide updated; translations untouched (R15).
- [ ] Fix vs feature: **feature** → minor bump + CHANGELOG.
- [ ] No "cobolt" in user-facing text; theme ids/names are product terms.
- [ ] Special-theme assets are **original** (R16) — operator-provided.

## 8. Phasing (proposed for /tasks)
- **Phase 1 — Unify the renderer.** Finish porting `paint::draw_control` and route
  the designer through it; **liquid-glass pixel-regression** green. (No visible
  change; prerequisite for R5.)
- **Phase 2 — Theme model + selection.** `FormTheme`/`ThemeCatalog`,
  `Form.theme` + project default, resolution, designer/Settings pickers, i18n.
  Only `liquid-glass` active. (R1–R4, R9; AC1–AC2, AC6.)
- **Phase 3 — Asset-pack engine.** `theme.toml` format, 9-slice rendering +
  texture cache in `draw_control`, fallback (R11), optional themed background
  (R8), and the **chart-style hook** (themed palette/stroke/material fill for
  pie/line/bar data marks); validate with **one** reference pack. (R6, R7, R8,
  R10; AC5, AC7.)
- **Phase 4 — The four special packs.** Import the **AI-generated original** art
  (steel, wood, clay, wool), slice it per control/state, and tune fidelity to the
  references. (R6; AC3.) *Consumes operator-provided generated assets.*
- **Phase 5 — Desktop + WASM parity & finalize.** Embed theme assets in the
  desktop binary + wasm bundle (R13, coordinate with 006); docs (R15); version
  bump/CHANGELOG; full AC walkthrough. (R5, R13; AC4, AC8.)
