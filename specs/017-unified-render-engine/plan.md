# Plan — Unified form rendering engine

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-21

## 1. Approach

Introduce one engine in `cobolt-forms` that renders a whole form, and migrate the
four existing loops to call it — **phased**, deleting each old path only once its
replacement is verified (R1, R8). The engine reuses the existing primitives
(`draw_control`, `draw_chart_preview`, `draw_glass`, `corner_radius`, the
`containers` helpers); it unifies the **loop and the interactive widgets**, which
are what diverged.

### The engine (new `crates/cobolt-forms/src/render.rs`, behind `render`)

```rust
pub trait FormState {                 // live values, source-agnostic (R4)
    fn prop(&self, id: &str, key: &str) -> Option<String>;
    fn visible(&self, id: &str) -> bool { true }
    fn enabled(&self, id: &str) -> bool { true }
}

pub enum RenderMode { Static, Interactive }   // R5

pub struct RenderInput<'a> {
    pub controls: &'a [Control],      // real Controls (R-Q2)
    pub state: &'a dyn FormState,
    pub glass: bool,
    pub theme: Option<ThemePack>,
    pub mode: RenderMode,
    pub active_tabs: &'a ActiveTabs,
}

#[derive(Default)]
pub struct RenderOutput {
    pub events: Vec<UiEvent>,                  // neutral; caller maps (R-Q3)
    pub prop_updates: Vec<(String,String,String)>, // (id,key,value)
    pub control_rects: HashMap<String, egui::Rect>, // for designer overlay (R6)
}

pub struct UiEvent { pub ctrl_id: String, pub event: String, pub value: Option<String> }

pub fn render_form(ui: &mut egui::Ui, input: &RenderInput) -> RenderOutput;
```

`render_form` owns, **once** (R3): the form background (the unset/black ⇒ navy
rule + background image scaling), the glass `ctx` visuals, the render/tree order,
per-control geometry (live `X/Y/Width/Height` from `state`), container clipping +
ancestor opacity + tab visibility, corner-radius/rounded rendering (spec 016),
the face via `draw_control`, charts via `draw_chart_preview`, images via the
rounded textured `RectShape`, and — in `Interactive` mode — the editable widgets
(TextBox, ComboBox + popup, ListBox, Slider, NumericUpDown, DateTimePicker +
calendar, TabControl strip, TreeView, Timer tick), accumulating `events` +
`prop_updates` and recording each control's screen rect.

### Callers (each reduced to: build state → call engine → handle output)

- **Preview** (`show_preview_window`): wrap the preview state map in a `FormState`;
  call `render_form(Interactive)`; apply `prop_updates` back to the map. Delete the
  preview control loop + its per-type branches.
- **Run** (`show_running_form_window`): wrap `CtrlState` in a `FormState`; call the
  engine; map `UiEvent` → `cobolt_runtime::FormEvent` and `send_event`; apply
  `prop_updates`. Delete `render_run_control` and the run loop.
- **Designer** (`designer.rs`): wrap the designed form values in a `FormState`;
  call `render_form(Static)`; then draw the **overlay** (selection handles,
  secondary highlights, rubber-band, repeating-group badge + preview clones,
  drop hints, grid, animation transforms) using `control_rects` (R6). Delete the
  designer control loop.
- **Compiled binary** (`cobolt-compiler` `FormApp::update`): wrap compiled state in
  a `FormState`; call the engine; map events. Delete its inline loop (R10).

## 2. Affected crates / files
- `crates/cobolt-forms/src/render.rs` — **new** engine + `FormState`/`UiEvent`/
  `RenderInput`/`RenderOutput`; headless invariant tests.
- `crates/cobolt-forms/src/` — move the container helpers (render order, clip,
  ancestor opacity, visibility, descendants, content rect) here from the IDE so
  the engine and designer share one copy (`containers` module); re-export for the
  IDE to avoid a churn of call sites.
- `crates/cobolt-ide/src/panels/designer.rs` — replace the draw loop with an
  engine call + overlay; keep editing/hit-testing.
- `crates/cobolt-ide/src/app.rs` — `show_preview_window` and
  `show_running_form_window` call the engine; **remove** `render_run_control` and
  the per-type branches.
- `crates/cobolt-ide/src/panels/containers.rs` — becomes a thin re-export of the
  `cobolt-forms` helpers (or is removed, call sites repointed).
- `crates/cobolt-compiler/src/lib.rs` — `FormApp::update` calls the engine.
- `docs/developers-guide-en.md` — short "one renderer" note.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — finalize.

## 3. Data / model changes
- **No `.cfrm`/model change.** New types are render-time only (`FormState`,
  `UiEvent`, `RenderInput/Output`) in `cobolt-forms`.
- **No dependency cycle:** the engine's `UiEvent` is defined in `cobolt-forms`;
  the run caller maps it to `cobolt_runtime::FormEvent` (cobolt-forms does **not**
  depend on cobolt-runtime).
- **Textures:** PictureBox/Animator images load via the egui `Context` cache the
  engine uses (as `draw_picturebox` already does), so no caller-side texture
  plumbing is needed; confirm in Phase 2.

## 4. Key decisions & alternatives
- **Engine takes `&[Control]`, not stringified meta (Q2)** — Why: the run/compiled
  paths currently rebuild a `Control` from string props before charting, which is
  the source of the washed-out chart; building real `Control`s once removes that
  class of bug. Rejected: a `CtrlMeta`-based engine (perpetuates the divergence).
- **Neutral `UiEvent` in cobolt-forms (Q3)** — Why: keeps the shared crate free of
  a `cobolt-runtime` dependency. Rejected: depending on cobolt-runtime (cycle/
  layering violation).
- **Designer editing stays an overlay (Q4)** — Why: the engine renders *the form*;
  selection/handles/badges/clones are an editor concern drawn on top using the
  returned `control_rects`. Rejected: baking editor chrome into the engine.
- **Phased cut-over with old paths kept until verified (Q5)** — Why: a big-bang
  swap of four surfaces is high-risk. Rejected: delete-then-rewrite.
- **Move `containers` helpers into cobolt-forms** — Why: the engine needs them and
  the designer already uses them; one copy prevents order/clip drift.

## 5. Risks & mitigations
- **Interactive widgets need `&mut Ui` + state mutation** → the engine takes
  `&mut egui::Ui` and returns `prop_updates`; callers apply them (matches today's
  `RunOutcome` pattern), so no interior mutability of caller state.
- **Behaviour regressions in events/popups/timer** (R9) → port the existing
  `render_run_control` + preview branches **verbatim** into the engine first
  (same code, one home), then migrate callers; AC4 checks each interaction.
- **Designer parity** (animations, selection, clones) → overlay uses
  `control_rects`; animation transforms can be passed via the `FormState`/input
  or applied in the overlay; verify the designer still matches before deleting its
  loop.
- **Borrow/ownership churn in `app.rs`/`designer.rs`** → the phased order
  (preview → run → designer → compiler) isolates each migration to one surface.
- **Compiler crate gains the engine** → it already depends on `cobolt-forms`;
  ensure the `render` feature is enabled there.
- **Big refactor, hard to fully pixel-test** → headless invariant tests
  (background colour, chart glass branch, control rects, rounding) per Q6; visual
  side-by-side per AC1/AC2.

## 6. Test strategy
- **`cobolt-forms` (render, headless egui Context)** — render the reference form
  (Panel ⊃ AreaChart, PictureBox, TextBox, ComboBox, rounded control) through the
  engine and assert invariants: background fill colour (navy for unset), the chart
  takes the **glass** branch when `glass=true`, control screen rects match the
  designed geometry, rounded controls emit rounded shapes. Report each assertion.
- **`cobolt-forms`** — `FormState` adapters: a control's live `X/Y/Width/Height`
  override is honoured; `visible/enabled` respected.
- **Interaction unit tests** — reuse the existing run-simulation harness (drives
  the engine in `Interactive` mode): TextBox change/focus/key, ComboBox select,
  Slider drag, Button click, CheckBox toggle, Timer tick still produce the same
  events/updates (AC4).
- **`cargo test --workspace`** green; **i18n** parity holds.
- **Manual** — side-by-side designer / preview / running form of the reference
  form (AC1–AC3, AC5); build a binary and compare to the IDE (AC7).

## 7. Steering compliance
- [x] i18n: no new labels expected; any added are `Tr` ×6.
- [x] Generated-code banner + regenerate-on-action unchanged (compiler swaps its
  loop for an engine call; banner/regenerate intact).
- [x] English dev guide updated (translations untouched).
- [x] Fix vs feature: internal refactor for parity → **fix** per the standing
  directive; finalize with a version bump + CHANGELOG.
- [x] No "cobolt" in user-facing text; COBOL identifiers/source English.
- [x] Reuse over reinvention: engine reuses `draw_control`, `draw_chart_preview`,
  glass, `corner_radius`, and the `containers` helpers — it unifies the loop.
