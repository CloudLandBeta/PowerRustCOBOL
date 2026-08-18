# Tasks — Chart multi-series data and interaction

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-08-18

Ordered so the tree stays green after every task. T1–T4 are pure plumbing and
change no picture; T5 is the non-regression gate; T6 (the zero baseline) lands
before the stacking tasks that depend on it; interaction (T11–T12) last.

Branch: `fix/chart-multi-series` — **FIX**, not a feature (plan §7, operator
decision 2026-08-18: these properties were published as complete). Announced on
**f=97**, no thread prefix.

---

- [ ] **T1 — The resolver, its types, and the value range** (R1, R6, R21)
  - Files: `crates/cobolt-forms/src/chart_data.rs` (new),
    `crates/cobolt-forms/src/lib.rs`
  - Do: add `ChartSeries`, `ChartData` (`categories`, `series`, `bubble`,
    `is_sample`), `ChartData::value_range() -> (f32, f32)` guaranteeing
    `min <= 0 <= max`, and `resolve(ctrl) -> ChartData` implementing the
    four-level precedence of plan §1.2. Parse `__ChartSeries` (header line of
    names, then `label` + one value per series) and `__ChartData`
    (`label<TAB>value`). **No cap** — every series given is carried. Nothing
    calls it yet.
  - Verify: `cargo test -p cobolt-forms --features render chart_data` green.
    Tests report, per fixture, which precedence level resolved, how many series
    came out, and the computed value range. Covers **AC15** (more series than
    palette colours ⇒ all drawn, none dropped).

- [ ] **T2 — Resolver precedence tests** (R2, R3, R4)
  - Files: `crates/cobolt-forms/src/chart_data.rs` (tests)
  - Do: fixtures for all four levels, including a control carrying **both**
    `__ChartSeries` and a contradicting `ValueFields`.
  - Verify: `cargo test -p cobolt-forms --features render chart_data` green.
    Covers **AC1**, **AC2** (live data outranks the properties), **AC3**.

- [ ] **T3 — Runtime: series-aware chart store** (R1, R5, R20)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: widen `chart_data` to hold category labels + N named series.
    `push_chart_data` emits `__ChartData` **byte-identically** for one unnamed
    series, `__ChartSeries` otherwise. `COBOL-CHART-ADD-POINT` and
    `COBOL-CHART-SET-TABLE` keep their signatures.
  - Verify: `cargo test -p cobolt-runtime chart` green —
    `test_chart_inline_methods` must pass **unchanged**: the non-regression proof
    for R5/R20.

- [ ] **T4 — Runtime: `COBOL-CHART-SET-SERIES` and `refresh_chart_binding`** (R2)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: add `COBOL-CHART-SET-SERIES` (series name + table). Add
    `refresh_chart_binding(control_id)` mirroring `refresh_datagrid_binding`:
    read `_BindingKind` / `_BindingFields`, one series per `ChartValueSeries`
    mapping, categories from `ChartCategory`, names from `ChartSeriesLabel`
    falling back to `series_id`.
  - Verify: `cargo test -p cobolt-runtime chart` green; new tests assert the
    emitted `__ChartSeries` text and report the series names and point counts.

- [ ] **T5 — Painter reads the resolver** (R1, R11, R12, R19)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: replace the inline `__ChartData` parse in `draw_chart_preview` with
    `chart_data::resolve`. Legend entries take `series[i].name`; pie/donut draw
    `series[0]` only.
  - Verify: `cargo test -p cobolt-forms --features render` green.
    **This is the AC4 gate:** a chart fed only `__ChartData` must paint the same
    shape set as before — assert it and report both counts. Covers **AC4**,
    **AC9**, **AC11**. The `elegance_baseline` must **not** move here; if it
    does, stop and find out why before touching it.

- [ ] **T6 — The zero baseline and negative values** (R21, R22, R23)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: derive `zero_y` and the signed `px_y` from `value_range()` per plan
    §1.2b. Bars run from `zero_y` to `px_y(v)` in whichever direction that takes
    them; the X axis line moves from the plot floor to `zero_y`. Line, area and
    scatter points use the same mapping. **No clamping** — a negative value is
    drawn below the axis (operator, Q5).
  - Verify: `cargo test -p cobolt-forms --features render chart` green.
    **AC19 is the sharp one:** an all-positive chart must be geometrically
    identical to the pre-change capture — compare **rect by rect**, not just the
    shape count, because a sub-pixel shift would pass a count check. Covers
    **AC17** (bar below the axis, axis at zero) and **AC19**. Report the measured
    `zero_y` for an all-positive, a mixed and an all-negative fixture.

- [ ] **T7 — Stacked bars, signed** (R7, R8, R22)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: with `Stacked` on and >1 series, one bar per category with **two** running
    totals — positives upward from `zero_y`, negatives downward — so a category
    holding both shows both.
  - Verify: `cargo test -p cobolt-forms --features render chart` green; the test
    reports the measured segment rects for stacked vs grouped. Covers **AC5**,
    **AC18**, and **AC7** for bar's siblings (no branch added ⇒ nothing changes).

- [ ] **T8 — Stacked areas, signed** (R7, R8, R22)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: each band's baseline is the previous band's running total on its own side
    of zero.
  - Verify: as T7; reports the upper-edge Y per category. Covers **AC6**;
    re-check **AC7**.

- [ ] **T9 — Bubbles** (R9, R10)
  - Files: `crates/cobolt-forms/src/paint.rs`, `chart_data.rs`
  - Do: carry `bubble` through the resolver (binding-sourced only — Q4); scatter
    radii interpolate from a visible floor to `BubbleScale`; empty `BubbleField`
    keeps `PointRadius`.
  - Verify: `cargo test -p cobolt-forms --features render chart` green; reports
    the radii in field order and asserts the largest equals `BubbleScale`.
    Covers **AC8**.

- [ ] **T10 — Re-bless the paint baseline** (R19)
  - Files: `crates/cobolt-forms/src/paint.rs` (`elegance_baseline_tests`)
  - Do: run it, and **account for the delta per control family before writing any
    number down** — as 1.61.97 did (+40 = 4 charts × 4 + 2 × 12). Record the
    arithmetic in the comment. If the delta cannot be explained, the cause is a
    bug, not a baseline that needs blessing.
  - Verify: `cargo test -p cobolt-forms --features render elegance_baseline`
    green, with the reasoning written above the table.

- [ ] **T11 — Hover tooltips** (R13, R14, R17, R18)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: record each element's hit rect while drawing; pointer from
    `painter.ctx().pointer_hover_pos()`; paint the chart's **own** rounded box +
    text (category, series name, value) for the element under it. Suppress while
    a pointer button is down (Q3). No signature change.
  - Verify: `cargo test -p cobolt-forms --features render chart` green, asserted
    through **both** the canvas path (`painted_text`) and the interactive path
    (`drive_painted`). Covers **AC10**, **AC11**, half of **AC14**.

- [ ] **T12 — Animation, triggered by new data** (R15, R16, R17, R18)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: `ctx.data` holds `(data_fingerprint, start_instant)` per control id.
    Fingerprint the resolved data each frame; a change restarts the animation.
    `t = ((now - start)/DURATION).clamp(0,1)` scales the distance **from
    `zero_y`**, so a negative bar grows downward as a positive grows upward.
    `ctx.request_repaint()` while `t < 1`.
  - Verify: `cargo test -p cobolt-forms --features render chart` green — capture
    at t=0 vs t=DURATION and report the measured bar heights; then assert that
    **moving, resizing and re-hovering** the chart does not restart it while
    **pushing new data does**. Covers **AC12**, **AC13**, the rest of **AC14**.
    Report the fingerprint cost (plan §5 risk).

- [ ] **T13 — IDE: bound charts preview on the canvas** (R2)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: add the `BindingTargetDescriptor::Chart` arm to
    `refresh_data_binding_target_properties`, mirroring the existing `DataGrid`
    arm; write a preview `__ChartSeries`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide databinding` green; a test
    asserts a bound chart gains the preview property and an unbound one does not.
    Covers **AC2** end-to-end.

- [ ] **T14 — System KB + chunked store** (AC16)
  - Files: `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`
  - Do: rewrite the eight property entries to describe delivered behaviour; add
    `COBOL-CHART-SET-SERIES`; state that there is **no** series cap, that
    `BubbleField` needs a binding, and that negative values draw below the axis.
    Then `cargo run --release -p cobolt-ide --example build_chunked_kb` and commit
    the regenerated store.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches`
    green. Covers **AC16**.
  - ⚠️ Edit with the Edit tool or ASCII-only splices — a scripted substitution
    carrying a non-ASCII escape re-encodes the whole file (operator memory note;
    it happened twice on 2026-08-18).

- [ ] **T15 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`; `crates/cobolt-ide/src/i18n.rs` **only
    if** a string landed on the IDE side.
  - Do: document multi-series charts (binding first, properties as fallback), no
    series cap, stacking **including the signed rule**, negative values below the
    axis (and that an all-negative chart puts the axis at the top — plan §5),
    bubbles, tooltips, and the new-data animation trigger. English guide only —
    never touch `-es/-pt/-jp/-cn`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green. If no IDE
    string was added, state that explicitly rather than leaving it implied.

- [ ] **T16 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the fix number `z`; write the CHANGELOG entry naming what now works,
    the signed-axis rule, and that existing all-positive charts are untouched.
  - Verify: the full sweep —
    `cargo test -q --workspace --exclude cobolt-bench --features cobolt-forms/render --no-fail-fast -- --skip generated_binary_source_actually_compiles`,
    then `cargo test -p cobolt-compiler --lib generated_binary_source_actually_compiles`.
    Expect the two `test_external_crates_e2e` failures (environmental —
    `libsqlite3-sys` will not compile in its nested build here); anything else is
    real. Then the manual check in plan §6: bind a two-field table, tick
    `Stacked`, feed a negative value and see it below the axis, hover a bar under
    Run Form, push new data and watch it animate once.

## Done criteria

All 19 acceptance criteria in `spec.md` checked, the full sweep green apart from
the two known environmental failures, the English guide and the System KB current,
and the work committed as a **fix** on `fix/chart-multi-series` — separate from any
feature commit, announced on **f=97** with no prefix. Do **not** commit, merge or
push unless the operator asks; the push window and announcement rules in
CLAUDE.md still apply.

## Coverage map

| AC | Task(s) | AC | Task(s) |
|---|---|---|---|
| AC1 | T2, T4 | AC11 | T5, T11 |
| AC2 | T2, T13 | AC12 | T12 |
| AC3 | T2 | AC13 | T12 |
| AC4 | T3, T5 | AC14 | T11, T12 |
| AC5 | T7 | AC15 | T1 |
| AC6 | T8 | AC16 | T14 |
| AC7 | T7, T8 | AC17 | T6 |
| AC8 | T9 | AC18 | T7 |
| AC9 | T5 | AC19 | T6 |
| AC10 | T11 | | |
