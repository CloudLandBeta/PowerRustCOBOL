# Tasks — Comprehensive fireable events for all controls

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-06-29

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

---

- [ ] **T1 — Define base event group constants** (R1)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: Add `const` arrays for each event category: `BASE_MOUSE` (11 events),
    `BASE_FOCUS` (2), `BASE_KEYBOARD` (5), `BASE_HOVER` (3), `BASE_GEOMETRY` (6),
    `BASE_DRAG` (7), `BASE_LIFECYCLE` (2). Place them above `supported_events()`.
  - Verify: `cargo check -p cobolt-forms` green. Constants compile but are
    not yet used.

- [ ] **T2 — Expand supported_events() for all visual controls** (R1, R2, R3.5, R3.6)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: Rewrite each `ControlType` match arm in `supported_events()` to return
    the full event list (base groups + type-specific events). Use `const` arrays
    per control type built from the group constants. Keep existing event names
    (e.g. `onDblClick`) and add aliases (`onDoubleClick`). Non-visual controls
    (Timer, AgentObject, RestClient, SqlDatabase) stay UNCHANGED. Add unit tests:
    `button_has_base_events`, `textbox_has_keyboard_events`, `timer_unchanged`,
    `agent_unchanged`, `picturebox_no_keyboard`.
  - Verify: `cargo test -p cobolt-forms --lib` all pass. Existing tests
    unaffected. **Covers AC1, AC4, AC11, AC13.**

- [ ] **T3 — Fire onRightClick, onMiddleClick, onContextMenu** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In `control_pointer_events()`, add detection for secondary click
    (`i.pointer.secondary_clicked()`) and middle click. When secondary click
    detected and over the control: fire `onRightClick` and `onContextMenu`.
    When middle click detected: fire `onMiddleClick`. Use `want()` check.
  - Verify: `cargo check -p cobolt-forms --features render` green. Add test
    `right_click_fires_event`. **Covers AC2.**

- [ ] **T4 — Fire onMouseMove, onMouseWheel** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In `control_pointer_events()`, when pointer is over the control:
    fire `onMouseMove` if `want("onMouseMove")` and pointer has moved.
    Fire `onMouseWheel` if `want("onMouseWheel")` and `i.scroll_delta != 0`.
  - Verify: `cargo check -p cobolt-forms --features render` green.

- [ ] **T5 — Fire onDoubleClick alias alongside onDblClick** (R3.6)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In `control_pointer_events()`, where `onDblClick` fires, also fire
    `onDoubleClick` if `want("onDoubleClick")`.
  - Verify: `cargo check -p cobolt-forms --features render` green.

- [ ] **T6 — Fire onKeyDown, onKeyUp, onKeyPress, onEnterPressed, onEscapePressed** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: Add a new function `control_keyboard_events()` called from
    `render_interactive()` for focusable controls. Check `ui.input(|i| i.events)`
    for `Event::Key` with `pressed: true/false`. Fire `onKeyDown`/`onKeyUp`.
    Fire `onKeyPress` for character keys. Fire `onEnterPressed` when
    Enter/Return detected. Fire `onEscapePressed` when Escape detected.
    Use `want()` check for each.
  - Verify: `cargo check -p cobolt-forms --features render` green.
    **Covers AC5.**

- [ ] **T7 — Fire onHoverEnter, onHoverLeave** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In `control_pointer_events()`, track hover duration using egui temp
    data keyed by `ctrl_id.with("hover-start")`. When pointer is over the
    control continuously for >200ms, fire `onHoverEnter` once. When pointer
    leaves after hovering, fire `onHoverLeave`. Reset timer on leave.
  - Verify: `cargo check -p cobolt-forms --features render` green.
    **Covers AC10.**

- [ ] **T8 — Fire onGotFocus, onLostFocus for all focusable controls** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: Currently only TextBox fires these. Move the focus-tracking pattern
    (using `resp.gained_focus()` / `resp.lost_focus()`) into
    `control_pointer_events()` or a shared helper, so ALL focusable controls
    (Button, CheckBox, RadioButton, ComboBox, ListBox, Slider, NumericUpDown,
    DateTimePicker, TreeView, DataGrid) fire `onGotFocus` and `onLostFocus`.
  - Verify: `cargo check -p cobolt-forms --features render` green.

- [ ] **T9 — Fire onVisibleChanged, onEnabledChanged** (R1, R3.4)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In `render_form()` or `render_interactive()`, track previous
    visible/enabled state per control in egui temp. When the state changes
    (either via IDE property set or runtime `SetProperty`), fire the
    corresponding event. Only fire when `want()` returns true.
  - Verify: `cargo check -p cobolt-forms --features render` green.
    **Covers AC9.**

- [ ] **T10 — Fire onResize, onResized, onMove, onMoved** (R1, R3.2, R3.3)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: Track previous rect (x, y, w, h) per control in egui temp. When size
    changes: fire `onResize` on each frame the size differs from the previous.
    When size stabilises (same for 2+ frames after changing): fire `onResized`
    once. Same pattern for position → `onMove` / `onMoved`.
  - Verify: `cargo check -p cobolt-forms --features render` green.
    **Covers AC3.**

- [ ] **T11 — Fire onLoad** (R1, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: Track "loaded" flag per control in egui temp. On the first frame a
    control is rendered, fire `onLoad` if `want("onLoad")`. Set the flag so
    it doesn't fire again.
  - Verify: `cargo check -p cobolt-forms --features render` green.

- [ ] **T12 — Fire type-specific new events** (R2, R3.1)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: For each control type's new events, add firing logic in the
    appropriate `CT::*` match arm in `render_interactive()`:
    - TextBox: `onTextChanged` (on any text change including programmatic),
      `onSelectionChanged`
    - ComboBox: `onDropDownClosed`
    - Slider: `onValueChanged` (on mouse release after change)
    - ProgressBar: `onValueChanged`, `onCompleted` (when value == maximum)
    - PictureBox: `onImageLoaded`, `onImageError`
    - Animator: `onFrameChanged`, `onLooped`
    - GroupBox/Panel: `onScroll` (when AutoScroll content scrolls)
    - TabControl: `onTabChanged`, `onTabClick`
    - DataGrid: `onCellDoubleClick`, `onColumnResize`, `onColumnResized`,
      `onSort`, `onSelectionChanged`, `onScroll`
    - TreeView: `onNodeDoubleClick` (alias for onNodeDblClick), `onNodeSelect`
    - Charts: `onZoom`
  - Verify: `cargo check -p cobolt-forms --features render` green.
    **Covers AC6, AC7, AC8.**

- [ ] **T13 — Event data: generated WS items** (R4)
  - Files: `crates/cobolt-codegen/src/lib.rs`
  - Do: In the generated WORKING-STORAGE section, add the `WS-EVENT-DATA`
    group item with fields for mouse position, key code, modifiers, drag
    delta, and property name/value. Only generate when at least one control
    has events that use data (mouse, keyboard, drag, property-change).
  - Verify: `cargo check -p cobolt-codegen` green. Generate a test form and
    verify the WS items appear in the output.

- [ ] **T14 — Event data: populate WS items at dispatch time** (R4)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: In the runtime event dispatch loop (where `send_event()` is called),
    set the appropriate WS items before dispatching. For mouse events: set
    `WS-EVENT-MOUSE-X/Y` from the pointer position relative to the control.
    For key events: set `WS-EVENT-KEY-CODE` and modifier flags. Use the
    `UiEvent.value` field to pass the raw data from render.rs to app.rs.
  - Verify: `cargo check -p cobolt-ide` green.

- [ ] **T15 — Docs: developers-guide-en.md event tables**
  - Files: `docs/developers-guide-en.md`
  - Do: Update the "Events you can handle" section with the full event table
    per control type. Group events by category (Mouse, Focus, Keyboard,
    Hover, Geometry, Drag, Lifecycle, Type-specific). Include a table for
    each control type showing its supported events.
  - Verify: Read the section. Translations untouched.

- [ ] **T16 — Finalize: version bump, full test, manual check**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: Bump patch version (`z`). Add CHANGELOG entry under Fix. Run full
    `cargo test`. Launch the IDE and verify:
    - Button Events section shows 20+ events (not just 6)
    - Right-click on a Button at runtime → onRightClick handler fires
    - Hover over a Label for 1 second → onHoverEnter fires
    - Existing onClick handlers still work
    - Timer/AgentObject/RestClient/SqlDatabase event lists unchanged
  - Verify: `cargo test` all green. Manual checks pass. **Covers all ACs.**

---

## AC ↔ Task mapping

| AC | Covered by |
|----|------------|
| AC1 | T2 (supported_events expansion) |
| AC2 | T3 (onRightClick + onContextMenu firing) |
| AC3 | T10 (onResize/onResized firing) |
| AC4 | T2 (TextBox events) |
| AC5 | T6 (onEnterPressed firing) |
| AC6 | T12 (DataGrid new events) |
| AC7 | T12 (PictureBox onImageLoaded) |
| AC8 | T12 (GroupBox onScroll) |
| AC9 | T9 (onVisibleChanged firing) |
| AC10 | T7 (onHoverEnter/Leave firing) |
| AC11 | T2 (supported_events lists) |
| AC12 | N/A (no new UI strings needed — events render dynamically) |
| AC13 | T2 (existing events unchanged, tested) |

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and the
change is split into fix commit(s) per the operator's rules (do **not**
commit/push unless the operator asks).
