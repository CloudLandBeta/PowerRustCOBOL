# Spec — Unified form rendering engine (one renderer everywhere)

- **Status:** draft → approved
- **Folder:** specs/017-unified-render-engine/
- **Author:** Anthropic Claude Codex Agent (with Eslopes)   **Date:** 2026-06-21

## 1. Overview

A form currently renders through **four separate code paths** that each
re-implement the form/control draw loop:

1. **Form Designer** canvas — `crates/cobolt-ide/src/panels/designer.rs`.
2. **Live preview** — `crates/cobolt-ide/src/app.rs::show_preview_window`.
3. **Running (interpreted) form** — `app.rs::show_running_form_window` +
   `render_run_control`.
4. **Compiled binary** — `crates/cobolt-compiler/src/lib.rs::FormApp::update`.

They share the per-control face painter (`cobolt_forms::paint::draw_control`) but
each owns its **form-level** rendering: window background, glass visuals, the
control iteration order, container clipping / opacity / tab-scoping, and the
**interactive widgets** (TextBox editing, ComboBox popup, Slider drag, ListBox
scroll, chart reconstruction). Every divergence between them — washed-out charts,
hard-coded TextBox glass, a Panel drawn as a blue box, mismatched backdrops, a
stale glass flag — has been a separate bug fixed in one path but not the others
(versions 1.27.8–1.27.10). This is whack-a-mole by construction.

This feature replaces the four loops with **one rendering engine** in the shared
`cobolt-forms` crate. Every caller delegates to it, so the **same form + same
state always produces the same pixels**, whether shown in the designer, the
preview, the running form, or a compiled/web binary. Live values, interactivity,
and selection differ only by **parameters** to the one engine — never by a
re-implementation.

## 2. Goals / Non-goals

### Goals
- A single public engine, e.g. `cobolt_forms::render::render_form(...)`, that
  draws a **whole form** (background + all controls) into an egui `Ui`/`Painter`.
- **Pixel-identical** output across designer, preview, run, and compiled for the
  same form + state + flags.
- The engine owns, **once**: form background (incl. the black⇒navy default and
  background image), glass visuals, render/tree order, container clipping,
  ancestor opacity, tab-page visibility, the corner-radius/rounded rendering
  (spec 016), charts, images, and every interactive control widget.
- A **state provider** abstraction so live values come from any source
  (designer edit state, preview state, runtime `CtrlState`, compiled state).
- An **interaction mode** so the same engine is read-only (designer/static) or
  interactive (preview/run/binary, emitting events), and a **selection/overlay**
  hook the designer adds on top (handles, badges, rubber-band, preview clones).
- All four call sites reduced to: gather state → call the engine → handle
  returned events/overlays.

### Non-goals
- Changing how controls *look* (the goal is parity, not a restyle); the engine
  reuses the existing `draw_control` / `draw_chart_preview` / glass primitives.
- New controls, properties, or behaviours (rendering consolidation only).
- Replacing egui or adding a new GPU backend (it remains egui 0.29).
- The COBOL interpreter / event dispatch semantics (unchanged; the engine only
  *emits* UI events the existing dispatch already consumes).
- Designer-only editing affordances move *onto* the engine as an overlay hook but
  their behaviour is unchanged.

## 3. User stories
- As a developer, a form looks the same in the designer, the preview, and when I
  run it — charts, panels, text fields, images, rounded corners, glass, and
  background all match.
- As a developer, when I toggle glass or a theme, all three views change together.
- As a maintainer, I fix or restyle a control in **one** place and every surface
  updates.
- As a developer, the compiled binary renders the form identically to the IDE.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** there shall be **one** form-rendering engine in
  `cobolt-forms` that draws the form background and all controls; the designer,
  preview, running form, and compiled binary shall render **only** through it.
- **R2 (ubiquitous):** given the same form, the same control state, and the same
  flags (glass, theme, interaction mode), the engine shall produce **identical**
  output on every surface.
- **R3 (ubiquitous):** the engine shall own, in one place: window/form
  **background** (including the unset/black ⇒ default-navy rule and the optional
  background image + scaling), **glass** visuals, **render order** (container tree
  order), **container clipping**, **ancestor opacity**, **tab-page visibility**,
  **corner radius / rounded rendering** (spec 016), **charts**, **images**, and
  the **interactive widgets** (text, combo, list, slider, numeric, date, tabs,
  tree, etc.).
- **R4 (ubiquitous):** the engine shall obtain live control values through a
  **state-provider** abstraction (a trait or closure) so each caller supplies its
  own source (designer/preview/runtime/compiled) without changing the engine.
- **R5 (state):** while in **read-only** mode the engine shall draw faces and emit
  no events nor mutate state; while in **interactive** mode it shall host the
  editable widgets and **return the UI events** (clicks, changes, focus, key,
  tab-switch, combo-open) the existing dispatch consumes, plus any property
  updates (e.g. a TextBox edit) for the caller to apply.
- **R6 (optional):** where a caller is the **designer**, it shall be able to draw
  its editing overlay (selection handles, secondary highlights, rubber-band,
  repeating-group badge + preview clones, drop hints) **on top of** the engine
  output via a documented hook, without the engine knowing about editing.
- **R7 (constraint):** the engine shall live in `cobolt-forms` (the crate all four
  callers already depend on) behind the existing `render` feature; it shall not
  depend on `cobolt-ide` or `cobolt-compiler`.
- **R8 (constraint):** the four existing render loops and `render_run_control`
  shall be **removed/replaced** by calls to the engine (no parallel renderer left
  behind to drift again).
- **R9 (constraint):** behaviour the engine subsumes — events fired, property
  updates, timer ticks, ComboBox popups, calendar popups, animation playback —
  shall remain functionally equivalent (no regression in interactivity or events).
- **R10 (ubiquitous):** the compiled-binary template (`cobolt-compiler`) shall
  call the same engine, so a packaged app matches the IDE.
- **R11 (constraint):** any new user-facing strings shall be `Tr` ×6; the English
  dev guide shall note that all surfaces share one renderer; translations
  untouched.

## 5. Acceptance criteria
- [ ] **AC1 (R1,R2,R3)** — A reference form with a Panel, a GroupBox, an
  AreaChart, a PictureBox (with image), a TextBox, a ComboBox and a rounded
  control renders **identically** (background, glass, colours, rounding,
  clipping) in the designer, the preview, and the running form. *(Verified by a
  side-by-side and by a headless shape/þixel snapshot test where feasible.)*
- [ ] **AC2 (R2)** — Toggling **glass** updates the designer, preview, and running
  form together; a chart is vivid/dim **the same way** in all three.
- [ ] **AC3 (R3)** — The form background (unset/black ⇒ navy, plus a background
  image) is the same on all surfaces.
- [ ] **AC4 (R4,R5,R9)** — In the running form, a TextBox edits and fires
  change/focus/key events; a ComboBox opens and selects; a Slider drags; a Button
  fires onClick; a Timer ticks — all unchanged from today.
- [ ] **AC5 (R6)** — The designer still shows selection handles, the repeating-
  group badge + preview clones, and drag/drop hints, drawn over the engine output.
- [ ] **AC6 (R8)** — `render_run_control`, the preview control loop, the designer
  control loop, and the compiler control loop no longer exist as separate
  implementations (grep shows one engine).
- [ ] **AC7 (R10)** — A compiled/run binary of the reference form matches the IDE
  preview screenshot.
- [ ] **AC8 (R7,R11)** — `cobolt-forms` builds with the engine behind `render`;
  `cargo test --workspace` green; i18n parity holds; the guide notes the single
  renderer.

## 6. Constraints & steering check
- **i18n:** rendering consolidation adds no new labels expected; any that appear
  are `Tr` ×6.
- **Generated-code / regenerate contract:** unchanged; the compiled template
  swaps its inline loop for an engine call but the banner + regenerate behaviour
  stay.
- **Docs:** English guide gains a short "one renderer" note; translations
  untouched.
- **Fix vs feature:** large internal refactor with user-visible parity outcome →
  treated as a **fix** per the standing pre-production directive (patch/minor at
  finalize), no behaviour added.
- **Reuse:** the engine *reuses* `draw_control`, `draw_chart_preview`, glass,
  `corner_radius`, and the `containers` helpers — it unifies the **loop**, not the
  primitives. The `containers` helpers (render order, clip, opacity, visibility)
  should move to / be shared from `cobolt-forms` so the engine and designer use
  one copy.
- **No "cobolt" in user text; COBOL identifiers English.**

## 7. Open questions
- **Q1 — state provider shape:** a trait `FormState { fn get(&self, id, key) ->
  Option<&str>; fn visible(&self,id)->bool; fn enabled(&self,id)->bool; }` vs a
  closure set. *Recommendation: a small trait; implement it for designer state,
  preview state, runtime `CtrlState`, and compiled state.*
- **Q2 — control model vs CtrlMeta:** the engine needs control type + rect +
  parent + tab + designed props. Does it take `&[Control]` (designer/preview have
  the full `Form`) or a lighter `&[CtrlMeta]` (run/compiled)? *Recommendation: the
  engine takes `&[Control]`; run/compiled build `Control`s once from their meta +
  state (they nearly do already), eliminating the reconstruct-from-strings
  divergence that caused chart bugs.*
- **Q3 — interaction return type:** the engine returns a `RenderOutput { events:
  Vec<FormEvent>, prop_updates: Vec<(id,key,val)> }`. Confirm `FormEvent` (from
  `cobolt-runtime`) can be referenced from `cobolt-forms` without a cycle, or
  define a neutral event type in `cobolt-forms` the callers map. *Recommendation:
  define the engine's event/update types in `cobolt-forms` (no dependency on
  `cobolt-runtime`); callers translate to `FormEvent`.*
- **Q4 — designer specifics:** selection outline, handles, badges, preview clones,
  grid, rubber-band stay in the designer as an **overlay** after the engine call
  (R6). Confirm none of these need to be *inside* the engine. *Recommendation:
  overlay-only; the engine exposes per-control screen rects so the overlay can
  position handles.*
- **Q5 — migration order & risk:** phase the cut-over (engine + preview first,
  then run, then designer, then compiler) so each surface is validated before the
  next, rather than a big-bang replacement. *Recommendation: phased; keep the old
  path until its replacement is verified, then delete (R8).*
- **Q6 — pixel-parity testing:** can we assert parity headlessly (shape lists from
  the existing egui test Context) rather than only by eye? *Recommendation: add a
  headless test that renders the reference form through the engine and asserts key
  invariants (background colour, chart glass branch, control rects, rounding);
  full pixel diff is out of scope.*
