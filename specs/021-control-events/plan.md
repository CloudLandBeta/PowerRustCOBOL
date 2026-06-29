# Plan — Comprehensive fireable events for all controls

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-29

## 1. Approach

The implementation has two distinct parts:

### Part A: Expand `supported_events()` (designable events)

Update each control's `supported_events()` match arm in `model.rs` to include
the full set of events from the spec. This is purely additive — existing
events stay, new ones are appended. The properties panel's Events section
renders dynamically from `supported_events()`, so new events appear in the
IDE automatically with no UI code changes.

**Grouping strategy:** Define the base event lists as `const` arrays, then
concatenate them per control type using a helper. This avoids repeating 30+
events in every match arm.

```rust
const BASE_MOUSE: &[&str] = &[
    "onClick", "onDoubleClick", "onDblClick", "onRightClick", "onMiddleClick",
    "onMouseDown", "onMouseUp", "onMouseMove", "onMouseEnter", "onMouseLeave",
    "onMouseWheel", "onContextMenu",
];
const BASE_FOCUS: &[&str] = &["onGotFocus", "onLostFocus"];
const BASE_KEYBOARD: &[&str] = &[
    "onKeyDown", "onKeyUp", "onKeyPress", "onEnterPressed", "onEscapePressed",
];
const BASE_HOVER: &[&str] = &["onHoverEnter", "onHoverLeave", "onTooltipShow"];
const BASE_GEOMETRY: &[&str] = &[
    "onResize", "onResized", "onMove", "onMoved",
    "onVisibleChanged", "onEnabledChanged",
];
const BASE_DRAG: &[&str] = &[
    "onDragStart", "onDrag", "onDragEnd",
    "onDragEnter", "onDragLeave", "onDragOver", "onDrop",
];
const BASE_LIFECYCLE: &[&str] = &["onLoad", "onPropertyChanged"];
```

Each control type's `supported_events()` returns a `&'static [&str]` built
from the relevant base groups + type-specific events. Since Rust requires
static slices, the full lists are declared as `const` arrays per control
type (one `const` per type, concatenating the groups).

### Part B: Wire event firing in the runtime (R3)

Expand `control_pointer_events()` in `render.rs` to fire the new events
when their conditions are met. Each event category needs specific detection
logic:

**Mouse (already partially wired):**
- `onRightClick` / `onMiddleClick` — check `i.pointer.secondary_clicked()` /
  `i.pointer.middle_clicked()` (egui 0.29 API).
- `onMouseMove` — fire when `over && i.pointer.velocity() != Vec2::ZERO`.
- `onMouseWheel` — check `i.pointer.scroll_delta()`.
- `onContextMenu` — fire alongside `onRightClick`.
- `onDoubleClick` — fire alongside existing `onDblClick` (alias, R3.6).

**Focus (already partially wired):**
- `onGotFocus` / `onLostFocus` — already fired for TextBox; extend to all
  focusable controls using `resp.gained_focus()` / `resp.lost_focus()`.

**Keyboard:**
- `onKeyDown` / `onKeyUp` / `onKeyPress` — check `ui.input(|i| i.events)`
  for `Event::Key` events when the control has focus.
- `onEnterPressed` / `onEscapePressed` — check for specific key presses.

**Hover:**
- `onHoverEnter` — use egui temp storage to track hover duration; fire when
  the pointer has been over the control for >200ms.
- `onHoverLeave` — fire when the pointer leaves after hovering.
- `onTooltipShow` — fire when egui's tooltip system is about to display.

**Geometry:**
- `onResize` / `onResized` — these fire at RUNTIME when a control is resized
  programmatically (via `SetProperty('Width', ...)` etc.). Track previous
  size in egui temp; fire on change. `onResize` fires during continuous
  changes; `onResized` fires when the value stabilises.
- `onMove` / `onMoved` — same pattern for position changes.
- `onVisibleChanged` / `onEnabledChanged` — track previous visible/enabled
  state in egui temp; fire on change.

**Drag & Drop:**
- `onDragStart` / `onDrag` / `onDragEnd` — use egui's `Sense::drag()` on
  controls at runtime. Currently only the designer handles drag; at runtime,
  controls don't support drag by default. This requires adding
  `Sense::drag()` to controls that opt in (via a `Draggable` property).
- `onDragEnter` / `onDragLeave` / `onDragOver` / `onDrop` — detect when a
  dragged control enters/leaves another control's bounds.

**Lifecycle:**
- `onLoad` — fire once per control on the first frame it's rendered. Track
  "loaded" state in egui temp.
- `onPropertyChanged` — fire when any property is changed via
  `SetProperty` at runtime. Hook into the property-set path.

### Part C: Event data (R4)

Add event data to `UiEvent` by extending its `value` field to carry
structured data. For simple events, `value` is `None`. For mouse events,
encode `"x,y"`. For key events, encode `"keycode,shift,ctrl,alt,cmd"`.
The runtime decodes these into well-known WS items before calling the
handler.

Alternatively, use the existing `FormEvent` mechanism and set WS items
directly in the interpreter before dispatching. This is cleaner and
doesn't change the `UiEvent` struct.

**Decision:** Set WS items in the runtime dispatcher (resolves Q3). Add
new codegen WS items: `WS-EVENT-MOUSE-X`, `WS-EVENT-MOUSE-Y`,
`WS-EVENT-KEY-CODE`, `WS-EVENT-KEY-SHIFT`, `WS-EVENT-KEY-CTRL`,
`WS-EVENT-KEY-ALT`, `WS-EVENT-KEY-CMD`, `WS-EVENT-DRAG-DX`,
`WS-EVENT-DRAG-DY`, `WS-EVENT-PROP-NAME`, `WS-EVENT-PROP-VALUE`.

## 2. Affected crates / files

| File | Change |
|------|--------|
| `crates/cobolt-forms/src/model.rs` | Expand `supported_events()` for all visual control types; add base event group constants |
| `crates/cobolt-forms/src/render.rs` | Expand `control_pointer_events()` with new event detection; add hover timer, geometry tracking, drag handling |
| `crates/cobolt-codegen/src/lib.rs` | Add WS event-data items to generated WORKING-STORAGE; set them before dispatch |
| `crates/cobolt-ide/src/app.rs` | Set WS event-data items before `send_event()` at runtime |
| `docs/developers-guide-en.md` | Full event table per control type |

## 3. Data / model changes

### `supported_events()` — expanded lists

No structural change to the model. The `supported_events()` method on
`ControlType` returns `&'static [&str]`. The lists grow from ~6 events per
control to ~30+ events. The return type stays the same.

### `UiEvent` — unchanged

The `UiEvent` struct keeps its current fields. Event data is passed via WS
items, not through UiEvent.

### Generated WS items — new

```cobol
01 WS-EVENT-DATA.
   05 WS-EVENT-MOUSE-X    PIC S9(5) VALUE 0.
   05 WS-EVENT-MOUSE-Y    PIC S9(5) VALUE 0.
   05 WS-EVENT-KEY-CODE    PIC 9(5)  VALUE 0.
   05 WS-EVENT-KEY-SHIFT   PIC 9     VALUE 0.
   05 WS-EVENT-KEY-CTRL    PIC 9     VALUE 0.
   05 WS-EVENT-KEY-ALT     PIC 9     VALUE 0.
   05 WS-EVENT-KEY-CMD     PIC 9     VALUE 0.
   05 WS-EVENT-DRAG-DX     PIC S9(5) VALUE 0.
   05 WS-EVENT-DRAG-DY     PIC S9(5) VALUE 0.
   05 WS-EVENT-PROP-NAME   PIC X(50) VALUE SPACES.
   05 WS-EVENT-PROP-VALUE  PIC X(256) VALUE SPACES.
```

## 4. Key decisions & alternatives

**D1: Base event groups as const arrays vs per-type inline**
- Decision: `const` arrays for each category, concatenated per type.
- Why: DRY — 30 base events × 20+ control types = 600 entries avoided.
- Rejected: Inline arrays per type — massive duplication, hard to maintain.

**D2: Event data via WS items vs UiEvent.value**
- Decision: Set WS items before dispatch (resolves Q3).
- Why: COBOL handlers can read them directly. No serialisation/parsing needed.
  Matches PowerCOBOL's event-parameter model.
- Rejected: Encoding in UiEvent.value — requires parsing in COBOL.

**D3: onDoubleClick as alias for onDblClick**
- Decision: Fire BOTH (resolves Q1). `onDblClick` stays for backward compat.
- Why: New code uses `onDoubleClick`; existing code uses `onDblClick`. Both
  work.
- Rejected: Rename — would break existing handlers.

**D4: Drag & Drop at runtime only**
- Decision: Runtime only (resolves Q2).
- Why: Designer drag is an IDE operation. Form events are for user interaction.
- Rejected: Firing in designer — confuses IDE drag with user drag.

**D5: Non-visual controls excluded**
- Decision: Timer, AgentObject, RestClient, SqlDatabase keep their existing
  events unchanged.
- Why: They have no visual presence — mouse, hover, focus, drag, geometry
  events are meaningless.

## 5. Risks & mitigations

- **Risk:** 30+ events per control may clutter the Events section in the
  properties panel.
  → **Mitigation:** The panel already scrolls. Consider grouping events by
  category in a future UI pass (not in this spec).

- **Risk:** Firing onMouseMove every frame the pointer is over a control could
  flood the event loop.
  → **Mitigation:** Only fire when `want("onMouseMove")` — i.e., the developer
  has attached a handler. No handler = no overhead.

- **Risk:** Hover timer (200ms) needs per-control state tracking.
  → **Mitigation:** Use egui temp data keyed by control ID, same pattern as
  `press_mem` and `ptr-over` already in `control_pointer_events()`.

- **Risk:** Generated WS-EVENT-DATA items may conflict with user-defined names.
  → **Mitigation:** Prefix with `WS-EVENT-` which follows the existing
  convention (`WS-COBOL-EVENT-ID`, etc.).

## 6. Test strategy

### Unit tests (cobolt-forms)

- `model::tests::button_has_base_events` — verify Button's `supported_events()`
  includes onClick, onDoubleClick, onRightClick, onMouseDown, etc.
- `model::tests::textbox_has_keyboard_events` — verify TextBox includes
  onKeyDown, onKeyUp, onEnterPressed, etc.
- `model::tests::timer_unchanged` — verify Timer's events are ONLY `["onTick"]`.
- `model::tests::agent_unchanged` — verify AgentObject's events unchanged.

### Integration tests (cobolt-forms, render feature)

- `render::tests::right_click_fires_event` — simulate secondary click on a
  Button, verify `onRightClick` and `onContextMenu` are in the output events.
- `render::tests::hover_fires_after_delay` — simulate pointer over a control
  for >200ms, verify `onHoverEnter` fires.

### Manual / visual checks

1. Open a form with a Button → Events section shows 20+ events (not just 6).
2. Run the form → right-click the Button → `onRightClick` handler fires.
3. Run the form → resize a control programmatically → `onResized` handler fires.
4. Run the form → hover over a Label for 1 second → `onHoverEnter` fires.
5. Existing onClick handlers still work unchanged.

## 7. Steering compliance

- [ ] i18n: No new UI strings needed (event names render dynamically)
- [ ] Generated-code banner + regenerate-on-action contract preserved
      (codegen picks up new events from `supported_events()` on regenerate;
      new WS items added to generated WORKING-STORAGE)
- [ ] English dev guide updated (event tables per control type);
      translations untouched
- [ ] Fix vs feature: **fix** (events should have been fireable from the
      start) → bump `z`
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
- [ ] Backward compatible: existing events unchanged, new events additive
