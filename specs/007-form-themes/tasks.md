# Tasks — Form themes

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-19

Ordered, independently-verifiable tasks by the plan's phases (§8). The project
stays green and Liquid Glass stays pixel-identical after each task. Phase 1 (and
T10) **coordinate with spec 006's shared renderer** — same `draw_control`.

## Phase 1 — Unify the renderer (prerequisite, no visible change)

- [x] **T1 — One shared `draw_control`** (R5 prereq, R9)
  - Files: `crates/cobolt-forms/src/paint.rs` (complete the port — shadows, every
    control path, slider — to full designer fidelity); `crates/cobolt-ide/src/
    panels/designer.rs` + `crates/cobolt-ide/src/app.rs` (route **all** call sites
    — designer, preview, run, inline — through `cobolt_forms::paint::draw_control`;
    retire/delegate the divergent `panels::designer::draw_control`).
  - Do: a single renderer for every surface, no behaviour change yet.
  - Verify: `cargo build -p cobolt-forms -p cobolt-ide`; **liquid-glass
    regression** — a known form's painter ops/snapshot are unchanged; manual: the
    designer and Preview look identical to before.

## Phase 2 — Theme model + selection (Liquid Glass only)

- [x] **T2 — `FormTheme` / `ThemeCatalog` + per-form theme** (R1, R2, R3, R9)
  - Files: `crates/cobolt-forms/src/theme.rs` *(new)* (`FormTheme`
    procedural|asset-pack, `ThemeCatalog`, resolution helper); `crates/cobolt-
    forms/src/model.rs` (`Form.theme: Option<String>`, `Form.use_theme_background:
    bool`); `crates/cobolt-forms/src/xml.rs` (persist/load both, additive).
  - Do: catalog with built-in `liquid-glass`; resolution `form ?? project ??
    liquid-glass`.
  - Verify: `cargo test -p cobolt-forms` — catalog contains `liquid-glass`;
    resolution test; `.cfrm` round-trip (absent ⇒ `None`/defaults).

- [x] **T3 — Project default theme in the manifest** (R3)
  - Files: `crates/cobolt-ide/src/project_model.rs` (`[theme] default`, serde
    default → `liquid-glass`).
  - Verify: `cargo test -p cobolt-ide` — empty/absent ⇒ `liquid-glass`;
    `default = "…"` round-trips.

- [x] **T4 — Theme selection UI + live re-render** (R4, R14)
  - Files: `crates/cobolt-ide/src/panels/settings_form.rs` (project default
    picker); `crates/cobolt-ide/src/panels/designer.rs` (per-form override in the
    Appearance pane); `crates/cobolt-ide/src/app.rs` (resolve + pass theme to
    `draw_control`; re-render on change); `crates/cobolt-ide/src/i18n.rs` (theme
    picker, per-form override, "use theme background" ×6).
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n`. Manual:
    pick a theme (project / per-form) → the designer re-renders immediately;
    Liquid Glass unchanged.

## Phase 3 — Asset-pack engine (validated on one reference pack)

- [x] **T5 — Pack format + discovery** (R2, R10)
  - Files: `crates/cobolt-forms/src/theme.rs` (`ThemePack`: parse
    `assets/themes/<id>/theme.toml` — per-control/per-state image refs, 9-slice
    insets, optional background, palette/typography); catalog discovery of packs.
  - Verify: `cargo test -p cobolt-forms` — a sample pack is discovered and its
    `theme.toml`/slice metrics parse; appears in the catalog (extensibility, AC7).

- [x] **T6 — 9-slice rendering + fallback in `draw_control`** (R6, R7, R11, R12)
  - Files: `crates/cobolt-forms/src/paint.rs` (asset-pack path: 9-slice composite
    per control + per-state; texture cache keyed by `(theme, part)`; **fallback**
    to liquid-glass for uncovered controls; explicit `Foreground/BackgroundColor`
    applied on top).
  - Verify: `cargo test -p cobolt-forms` (slice math; fallback selection). Manual:
    a control with a pack image renders 9-sliced; an uncovered control falls back
    to glass.

- [x] **T7 — Themed background + chart-style hook** (R7 charts, R8)
  - Files: `crates/cobolt-forms/src/paint.rs` (optional themed background when
    `use_theme_background`; **chart-style hook** — pie/line/bar read the pack's
    palette + stroke + optional material **fill texture** instead of hard-coded
    colours).
  - Verify: `cargo test -p cobolt-forms` (chart-style resolution). Manual: bg
    toggle works (R8/AC5); PIE/LINE/BAR data marks take the theme palette/fill.

- [x] **T8 — One reference pack end-to-end** (R6)
  - Files: `assets/themes/<reference>/` (a minimal real pack: core controls +
    chart fills + background).
  - Verify: Manual: selecting it in the designer renders the core controls and a
    chart faithfully; proves the engine before the full art set.

## Phase 4 — The four special packs (operator-provided AI-generated art)

- [ ] **T9 — Stainless steel · dark wood · modeling clay · knitted wool** (R6, R16)
  - ⛔ **BLOCKED — needs operator-supplied AI-generated art.** The engine + pack
    format + a procedural reference pack (`cobalt-steel`) are done (T5–T8); these
    four packs only need their per-control/per-state 9-slice PNGs (+ backgrounds
    + chart fills) dropped into `assets/themes/{stainless-steel,dark-wood,
    modeling-clay,knitted-wool}/` with a `theme.toml` each. They then surface in
    the pickers automatically (AC7). **Framed** controls render with no code
    change; **sprite/composite** controls (checkbox, slider, combobox, …) need
    **Phase 6 (T13–T17)** first. I cannot author the raster art.
  - Files: `assets/themes/{stainless-steel,dark-wood,modeling-clay,knitted-wool}/`
    (manifests + per-control/per-state 9-slice art + chart fills + backgrounds),
    imported from the operator's AI-generated original assets and sliced/tuned.
  - Verify: Manual: each theme matches its reference mockup for panel/container,
    button, slider, label, and charts, incl. per-state (AC3). *Consumes
    operator-provided assets.*

## Phase 5 — Desktop + WASM parity & finalize

- [ ] **T10 — Embed + apply themes in desktop binary and WASM** (R5, R13)
  - ⛔ **BLOCKED — depends on spec 006.** The desktop/WASM build pipeline
    (`build_web_project`, the eframe binary's form loop) is spec 006 and not yet
    implemented; `cobolt-compiler` has no `draw_control` wiring. The shared
    renderer (T1) and the ctx-based active-theme mechanism are ready for it: the
    binary's render loop just calls `paint::set_active_theme` like the designer.
    Revisit when 006 lands.
  - Files: `crates/cobolt-compiler/src/lib.rs` (embed the resolved theme's assets
    in the desktop build and the wasm bundle; pass the theme to the generated
    render loop). Coordinates with spec 006.
  - Verify: build a desktop binary of a themed form → identical to the designer;
    (when 006 lands) the wasm build renders the same (AC4).

- [x] **T11 — Docs & i18n** (R14, R15; AC8)
  - Files: `docs/developers-guide-en.md` ("Form themes": catalog, project default +
    per-form override, themed background, adding packs); confirm all `Tr` keys ×6.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty). English guide only.

- [x] **T12 — Finalize** (all ACs) — version 1.26.0 + CHANGELOG + steering; full
      workspace tests green. (AC3 special-pack art and AC4 desktop/WASM parity
      remain pending their external deps — see T9/T10.)
  - Files: `crates/cobolt-ide/src/version.rs` (+ `CHANGELOG.md`) — feature minor
    bump; `specs/steering/product.md` (themable-forms capability note).
  - Verify: `cargo build --workspace` + `cargo test --workspace` green. Manual AC
    walkthrough: AC1 (catalog + select), AC2 (default + override + fallbacks), AC3
    (each special pack faithful), AC4 (designer = desktop = wasm), AC5 (themed
    background toggle), AC6 (existing forms = Liquid Glass), AC7 (drop-in pack
    appears), AC8 (i18n ×6 + docs).

## Phase 6 — Asset-pack engine extensions (decomposed special-theme assets)

Implements the gaps in [asset-decomposition.md](./asset-decomposition.md) §5 so the
engine can consume the **decomposed** photoreal assets (cobalt-steel first) for
**all** control kinds — not just single-image 9-slice frames. Each task keeps
Liquid Glass and the existing single-image packs working (additive), and the
reference pack / any delivered pack must still load. Order lets the project stay
green; T13 alone makes framed controls consume parts, the rest add the
non-9-slice control families.

- [ ] **T13 — Parts-mode 9-slice + per-edge tile/stretch** (addendum §5.1, §5.5)
  - Files: `crates/cobolt-forms/src/theme_pack.rs` (skin `mode = "parts"`: the 9
    part files `tl t tr l c r bl b br`; derive insets from corner sizes; optional
    `tile_edges`/`tile_center`/`grain`); `crates/cobolt-forms/src/paint.rs`
    (composite parts — corners fixed, edges/center stretched or tiled — reusing
    `nine_slice_cells`; build/cache the part textures).
  - Do: a framed control (button/panel/textbox/…) can be skinned from 9 separate
    PNGs and render identically to the single-image path.
  - Verify: `cargo test -p cobolt-forms --features render` — a parts-mode pack and
    an equivalent single-image pack produce the same 9 dest rects (geometry test);
    a missing part falls back cleanly. Report counts.

- [ ] **T14 — 3-slice frames** (addendum §5.2)
  - Files: `theme_pack.rs` (`mode = "hslice"`/`"vslice"`: `l c r` parts);
    `paint.rs` (3-region composite). Used by menubar/toolbar/statusbar/splitter and
    the slider track/fill + progressbar fill + tabcontrol tab.
  - Verify: `cargo test -p cobolt-forms` — horizontal/vertical 3-slice geometry
    (caps fixed, middle stretched/tiled); fallback to glass when absent.

- [ ] **T15 — Sprite controls: checkbox & radio** (addendum §5.3, §5.6)
  - Files: `theme_pack.rs` (`sprite` skin: fixed box/knob per state + `check`/`dot`
    glyph overlay); `paint.rs` (draw the box/knob left-aligned at fixed size +
    glyph when checked; label drawn by existing code); add `Checked` to
    `ControlState`.
  - Verify: `cargo test -p cobolt-forms` — a checkbox/radio skin resolves box +
    state + checked glyph; unskinned → glass. Manual: renders beside the caption.

- [ ] **T16 — Composite controls (sub-elements)** (addendum §5.4)
  - Files: `theme_pack.rs` (sub-element layout descriptors); `paint.rs`
    (slider: track+fill+thumb+ticks; progressbar: trough+fill; combobox/
    numericupdown/datetimepicker: field + right-edge button(s) + glyph;
    tabcontrol: body + tab strip; groupbox/datagrid: header band). Per-state
    thumbs/buttons.
  - Verify: `cargo test -p cobolt-forms` (layout math: thumb position, fill width,
    button rects). Manual: a themed slider/combo/progress matches the mockup;
    uncovered sub-parts fall back to glass.

- [ ] **T17 — Glyph/icon overlays + extra states** (addendum §5.6)
  - Files: `theme_pack.rs`/`paint.rs` — `arrow_down/up`, `expander_collapsed/
    expanded`, `calendar`, `sort_asc/desc` glyphs; `Selected` state (tab/list row);
    optional palette-tinting hook (§5.8).
  - Verify: `cargo test -p cobolt-forms` — glyph resolution per state; tint applies
    palette foreground when requested.

- [ ] **T18 — Reference/min-size metadata + cobalt-steel decomposed pack** (§5.7)
  - Files: `theme_pack.rs` (`reference_size`, `min_size`; clamp so a frame never
    shrinks below its insets); `assets/themes/cobalt-steel/` (import the operator-
    delivered decomposed parts per the addendum folder layout, replacing the
    procedural reference pack).
  - Verify: Manual — the cobalt-steel pack renders every control family
    (framed, sprite, composite) faithfully to the mockup at varied sizes with no
    seams/distortion; `cargo test -p cobolt-forms` for any size-clamp logic.

- [ ] **T19 — Docs + finalize (engine extensions)**
  - Files: `docs/developers-guide-en.md` (extend "Form themes → adding packs": the
    decomposed/parts + sprite + composite formats); `CHANGELOG.md` +
    `crates/cobolt-ide/src/version.rs` (minor bump).
  - Verify: `cargo test --workspace` green; `cargo test -p cobolt-ide i18n`.

> **Sequencing:** T13–T17 are engine work (no operator assets needed — unit-test
> with tiny fixtures). T18 consumes the delivered cobalt-steel zip and supersedes
> the procedural reference pack. T9 (the four special packs) then becomes a pure
> asset drop-in once T13–T17 land.

## Done criteria
All acceptance criteria are covered (AC1: T2/T4 · AC2: T2/T3 · AC3: T8/T9 · AC4:
T1/T10 · AC5: T7 · AC6: T1/T2/T12 · AC7: T5 · AC8: T11), tests pass, Liquid Glass
is unchanged, docs/steering updated, and the work is committed as feature
commit(s) per the operator's rules (do **not** commit/push unless asked). Phase 4
consumes the operator's AI-generated original packs; **Phase 6** adds the engine
support those decomposed packs require (asset-decomposition.md §5).
