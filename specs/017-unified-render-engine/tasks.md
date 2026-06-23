# Tasks — Unified form rendering engine

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-21

Phased cut-over: build the engine, then migrate one surface at a time, deleting
each old loop only after its replacement is verified. The project stays green and
launchable after every task.

### Phase A — engine foundation

- [x] **T1 — Move container helpers into `cobolt-forms`** (R3,R7)
  - Files: new `crates/cobolt-forms/src/containers.rs`;
    `crates/cobolt-ide/src/panels/containers.rs` → thin re-export.
  - Do: move `render_order`, `is_descendant`, `collect_descendants`, `is_visible`,
    `clip_rect`, `ancestor_opacity`, `resolve_drop_target`, `ActiveTabs` to
    cobolt-forms; re-export from the IDE module so existing call sites compile
    unchanged.
  - Verify: `cargo build -p cobolt-forms` + `cargo build -p cobolt-ide`;
    `cargo test -p cobolt-ide` green (container tests still pass).

- [x] **T2 — Engine skeleton: types + background + faces (Static)** (R1,R3,R4)
  - Files: new `crates/cobolt-forms/src/render.rs` (behind `render`); `lib.rs` mod.
  - Do: define `FormState`, `RenderMode`, `RenderInput`, `RenderOutput`,
    `UiEvent`. Implement `render_form` for the **Static** path: form background
    (unset/black ⇒ navy + bg image), glass `ctx` visuals, render-order loop with
    live geometry from `state`, clip + ancestor opacity + tab visibility, faces via
    `draw_control` / `draw_chart_preview` (real `Control`s) / rounded image; record
    `control_rects`. No interactive widgets yet.
  - Verify: `cargo test -p cobolt-forms --features render` green; headless test:
    reference form renders, background is navy for unset, chart takes the glass
    branch when `glass=true`, control rects match geometry.

- [x] **T3 — Engine interactive widgets** (R5,R9)
  - Files: `crates/cobolt-forms/src/render.rs`.
  - Do: port the **Interactive** widgets verbatim from `render_run_control` + the
    preview branches into the engine (Button press/hover, CheckBox/RadioButton,
    TextBox edit + focus/key, ComboBox header + popup, ListBox, Slider drag,
    NumericUpDown, DateTimePicker + calendar, TabControl strip, TreeView, Timer
    tick, PictureBox/Animator); accumulate `events` + `prop_updates`.
  - Verify: `cargo test -p cobolt-forms --features render` green; interaction-sim
    test drives the engine and sees TextBox change/focus/key, ComboBox select,
    Slider drag, Button click, CheckBox toggle, Timer tick.

### Phase B — migrate the surfaces (delete old loops)

- [x] **T4 — Preview → engine** (R1,R2,R8) — build + tests green; **manual visual
      parity pending operator sign-off**.
  - Files: `crates/cobolt-ide/src/app.rs::show_preview_window`.
  - Do: implement `FormState` over the preview state map; call
    `render_form(Interactive)`; apply `prop_updates`; map `UiEvent` as the preview
    needs. **Delete** the preview control loop + per-type branches.
  - Verify: `cargo build -p cobolt-ide`; manual: preview unchanged from today.
  - Done: added engine `RenderTransform` hook (animation shift/scale/alpha) +
    `PreviewState`; backdrop now owned by the engine; preview loop deleted.

- [ ] **T5 — Running form → engine; delete `render_run_control`** (R1,R2,R8,R9)
  - Files: `crates/cobolt-ide/src/app.rs::show_running_form_window`; remove
    `render_run_control`.
  - Do: implement `FormState` over `CtrlState`; call the engine; map `UiEvent` →
    `cobolt_runtime::FormEvent` + `send_event`; apply `prop_updates`. Delete the
    run loop and `render_run_control` (and its now-dead helpers).
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide` green
    (interaction-sim still passes); manual: running form interactive + **chart now
    matches the designer/preview** (the original bug).

- [ ] **T6 — Designer → engine + overlay; delete designer loop** (R1,R2,R6,R8)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`.
  - Do: implement `FormState` over the designed form; call `render_form(Static)`;
    draw the editor **overlay** (selection handles, secondary highlights,
    rubber-band, repeating-group badge + preview clones, grid, drop hints) using
    `control_rects`; keep hit-testing/selection/drag logic. Delete the designer
    control draw loop.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide` green; manual:
    canvas identical to before; selection/drag/badges/clones still work.

- [x] **T7 — Compiled binary → engine** (R1,R2,R10) — done out of phase order to
      fix a reported "binary has no styles" bug. Built `examples/scatter-chart` to
      a native binary and confirmed it renders the dark backdrop + glass chart
      (was all-white native widgets before).
  - Files: `crates/cobolt-compiler/src/lib.rs::FormApp::update`.
  - Do: implement `FormState` over compiled state; call the engine; map events.
    Delete the inline control loop. Ensure the `render` feature is on for
    `cobolt-forms` in the compiler.
  - Done: `CompiledState` `FormState` over the control-state map; engine-owned
    backdrop (color + bg image); `glass=true`; correct event names
    (`onClick`/`onChange`/…); input-sync channel wired (slider fix applies to the
    binary too). Inline native-widget loop deleted.
  - Verify: `cargo build -p cobolt-compiler`; build a packaged binary of the
    reference form and confirm it matches the IDE preview.

### Phase C — finalize

- [ ] **T8 — Parity test + docs** (R2,R11)
  - Files: `crates/cobolt-forms/tests/` (or `render.rs` tests);
    `docs/developers-guide-en.md`.
  - Do: add the headless reference-form parity/invariant test; add a short "one
    renderer for designer/preview/run/binary" note to the English guide.
  - Verify: `cargo test --workspace` green; section present; translations
    untouched.

- [ ] **T9 — Finalize** (all ACs)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`.
  - Do: version bump + CHANGELOG (fix per standing directive); confirm no
    `render_run_control` / parallel loop remains (grep). Fold/supersede the interim
    1.27.10 parity patches as appropriate.
  - Verify: `cargo build -p cobolt-ide` + `cargo build -p cobolt-compiler` +
    `cargo test --workspace` green; manual AC1–AC7 walkthrough (designer = preview
    = run = binary), incl. the chart.

## Done criteria
AC1 (T2,T4,T5,T6), AC2 (T2,T5,T6), AC3 (T2), AC4 (T3,T5), AC5 (T6), AC6 (T4–T7
+ T9 grep), AC7 (T7), AC8 (T8,T9). One engine renders every surface; the four old
loops are gone; tests pass; docs updated. Do **not** commit/push unless the
operator asks.
