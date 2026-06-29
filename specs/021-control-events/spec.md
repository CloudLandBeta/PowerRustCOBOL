# Spec — Comprehensive fireable events for all controls

- **Status:** draft
- **Folder:** specs/021-control-events/
- **Author:** Claude (spec-driven)   **Date:** 2026-06-29

## 1. Overview

Expand every control's supported events to include a comprehensive, consistent
set of **fireable** events — not just designable placeholders but events that
are actually dispatched by the runtime when the corresponding interaction or
state change occurs. Currently, most controls support only 4–8 events
(onClick, onDblClick, onMouseEnter/Leave). This spec standardises a full
event vocabulary across all visual controls, aligned with the form-level event
model and modern UI expectations.

Every event listed here MUST be actually fired by the runtime when the
condition occurs. An event that is designable but never fired is a bug.

## 2. Goals / Non-goals

### Goals

- Every visual control supports a consistent base set of events.
- Type-specific events are added only where the control's nature warrants them.
- All events are fireable at runtime (not just designable stubs).
- In-progress events (`onResizing`, `onDragging`) fire during the action;
  completion events (`onResized`, `onDragEnd`) fire when the action finishes.
- Event naming is consistent: `on` + verb (present = during, past = after).
- Backward compatible — existing events keep their current names and behavior.

### Non-goals

- Removing or renaming existing events (backward compatibility).
- Touch/gesture events (future spec).
- Accessibility events (future spec).

## 3. User stories

- As a **COBOL developer**, I want to handle right-click on any control so I
  can show custom context logic.
- As a **COBOL developer**, I want to know when a control is resized at
  runtime so I can adjust related controls.
- As a **COBOL developer**, I want drag-and-drop events so I can implement
  reorderable lists and file drops.
- As a **COBOL developer**, I want keyboard events on any focusable control
  so I can implement shortcuts and validation.
- As a **COBOL developer**, I want hover events on any control so I can
  implement dynamic UI feedback.

## 4. Requirements (EARS)

### R1 — Universal base events (all visual controls)

Every visual control (excluding Timer, AgentObject, RestClient, SqlDatabase)
shall support these base events. They are grouped by category.

**Mouse / Pointer:**
| Event | When fired |
|-------|-----------|
| `onClick` | Primary button clicked (press + release over the control) |
| `onDoubleClick` | Primary button double-clicked |
| `onRightClick` | Secondary button clicked |
| `onMiddleClick` | Middle button clicked |
| `onMouseDown` | Any mouse button pressed while over the control |
| `onMouseUp` | Any mouse button released while over the control |
| `onMouseMove` | Mouse moves while over the control |
| `onMouseEnter` | Mouse enters the control's bounds |
| `onMouseLeave` | Mouse leaves the control's bounds |
| `onMouseWheel` | Mouse wheel scrolled while over the control |
| `onContextMenu` | Right-click or context-menu key pressed |

**Focus:**
| Event | When fired |
|-------|-----------|
| `onGotFocus` | Control receives keyboard focus |
| `onLostFocus` | Control loses keyboard focus |

**Keyboard** (focusable controls only — TextBox, ComboBox, ListBox, DataGrid,
NumericUpDown, DateTimePicker, Slider, TreeView, Button, CheckBox,
RadioButton):
| Event | When fired |
|-------|-----------|
| `onKeyDown` | A key is pressed while the control has focus |
| `onKeyUp` | A key is released while the control has focus |
| `onKeyPress` | A character key is typed (after onKeyDown, before onKeyUp) |
| `onEnterPressed` | Enter/Return key pressed while the control has focus |
| `onEscapePressed` | Escape key pressed while the control has focus |

**Hover:**
| Event | When fired |
|-------|-----------|
| `onHoverEnter` | Pointer enters control bounds and remains stationary for >200ms |
| `onHoverLeave` | Pointer leaves control bounds after hovering |
| `onTooltipShow` | Tooltip is about to be displayed (can be used to set dynamic tooltip text) |

**Geometry:**
| Event | When fired |
|-------|-----------|
| `onResize` | Control is being resized (fires repeatedly during drag) |
| `onResized` | Control resize completed (fires once on mouse release) |
| `onMove` | Control is being moved (fires repeatedly during drag) |
| `onMoved` | Control move completed (fires once on mouse release) |
| `onVisibleChanged` | Control's Visible property changed |
| `onEnabledChanged` | Control's Enabled property changed |

**Drag & Drop:**
| Event | When fired |
|-------|-----------|
| `onDragStart` | User begins dragging the control |
| `onDrag` | Control is being dragged (fires repeatedly during drag) |
| `onDragEnd` | User releases the dragged control |
| `onDragEnter` | A dragged item enters this control's bounds |
| `onDragLeave` | A dragged item leaves this control's bounds |
| `onDragOver` | A dragged item moves over this control |
| `onDrop` | A dragged item is dropped onto this control |

**Lifecycle:**
| Event | When fired |
|-------|-----------|
| `onLoad` | Control is first rendered on the form |
| `onPropertyChanged` | Any property value changed programmatically |

### R2 — Type-specific events

In addition to the base events, certain controls support events specific to
their nature. These KEEP their existing names for backward compatibility.

**TextBox:**
| Event | When fired |
|-------|-----------|
| `onChange` | Text content changed (by user input) |
| `onTextChanged` | Text content changed (by user input OR programmatically) |
| `onSelectionChanged` | Text selection changed |
| `onEnter` | Focus entered (alias for onGotFocus, kept for compat) |
| `onLeave` | Focus left (alias for onLostFocus, kept for compat) |

**CheckBox / RadioButton:**
| Event | When fired |
|-------|-----------|
| `onCheckedChanged` | Checked state toggled |
| `onValueChanged` | Value changed (alias for onCheckedChanged) |

**ComboBox:**
| Event | When fired |
|-------|-----------|
| `onChange` | Selected item changed |
| `onSelectedIndexChanged` | Selected index changed |
| `onDropDown` | Dropdown opened |
| `onDropDownClosed` | Dropdown closed |

**ListBox:**
| Event | When fired |
|-------|-----------|
| `onChange` | Selection changed |
| `onSelectedIndexChanged` | Selected index changed |
| `onItemDoubleClick` | Item double-clicked |

**Slider:**
| Event | When fired |
|-------|-----------|
| `onChange` | Value changed (during drag) |
| `onValueChanged` | Value changed (on release — final value) |

**NumericUpDown:**
| Event | When fired |
|-------|-----------|
| `onChange` | Value changed |
| `onValueChanged` | Value changed (alias) |

**DateTimePicker:**
| Event | When fired |
|-------|-----------|
| `onChange` | Date/time value changed |
| `onValueChanged` | Value changed (alias) |

**DataGrid:**
| Event | When fired |
|-------|-----------|
| `onCellClick` | Cell clicked |
| `onCellDoubleClick` | Cell double-clicked |
| `onCellChange` | Cell value edited |
| `onRowSelect` | Row selection changed |
| `onColumnClick` | Column header clicked |
| `onColumnResize` | Column width changed |
| `onColumnResized` | Column resize completed |
| `onRowDoubleClick` | Row double-clicked |
| `onSelectionChanged` | Selection changed |
| `onScroll` | Grid scrolled |
| `onExportCSV` | CSV export completed |
| `onSort` | Column sort applied |

**TreeView:**
| Event | When fired |
|-------|-----------|
| `onNodeClick` | Node clicked |
| `onNodeDoubleClick` | Node double-clicked (renamed from onNodeDblClick) |
| `onNodeExpand` | Node expanded |
| `onNodeCollapse` | Node collapsed |
| `onNodeChecked` | Node checkbox toggled |
| `onNodeSelect` | Node selection changed |
| `onNodeDrag` | Node being dragged |
| `onNodeDrop` | Node dropped onto another node |

**PictureBox:**
| Event | When fired |
|-------|-----------|
| `onImageLoaded` | Image finished loading |
| `onImageError` | Image failed to load |

**Animator:**
| Event | When fired |
|-------|-----------|
| `onStarted` | Animation started playing |
| `onEnded` | Animation finished playing |
| `onFrameChanged` | Current frame changed |
| `onLooped` | Animation looped back to start |

**GroupBox / Panel:**
| Event | When fired |
|-------|-----------|
| `onScroll` | Container scrolled (when AutoScroll is on) |
| `onChildAdded` | A child control was added at runtime |
| `onChildRemoved` | A child control was removed at runtime |

**TabControl:**
| Event | When fired |
|-------|-----------|
| `onTabChanged` | Active tab changed |
| `onTabClick` | Tab header clicked |
| `onTabClosing` | Tab close requested (if closable tabs are added) |

**ProgressBar:**
| Event | When fired |
|-------|-----------|
| `onValueChanged` | Progress value changed |
| `onCompleted` | Value reached Maximum |

**MenuBar:**
| Event | When fired |
|-------|-----------|
| `onMenuClick` | Menu item clicked (passes item ID) |
| `onMenuItemClick` | Menu item clicked (passes item path) |
| `onMenuOpen` | Dropdown menu opened |
| `onMenuClose` | All menus closed |

**Non-visual controls** (Timer, AgentObject, RestClient, SqlDatabase) are
excluded from this spec. Their existing events remain unchanged — no base
events (R1) or new type-specific events are added to them.

**Charts (all types):**
| Event | When fired |
|-------|-----------|
| `onDataChanged` | Chart data updated |
| `onClick` | Chart area clicked |
| `onSeriesClick` | Data series/point clicked |
| `onTooltipShow` | Tooltip about to display |
| `onZoom` | Chart zoomed in/out |

### R3 — Firing contract

- **R3.1 (ubiquitous):** Every event listed in a control's `supported_events()`
  MUST be fired by the runtime when the corresponding condition occurs.
- **R3.2 (ubiquitous):** In-progress events (`onResize`, `onMove`, `onDrag`)
  shall fire on each frame/step during the action.
- **R3.3 (ubiquitous):** Completion events (`onResized`, `onMoved`, `onDragEnd`)
  shall fire exactly once when the action finishes.
- **R3.4 (ubiquitous):** Property-change events (`onVisibleChanged`,
  `onEnabledChanged`, `onPropertyChanged`) shall fire both when the property
  is changed via the IDE AND when changed programmatically at runtime.
- **R3.5 (constraint):** Events that are not applicable to a control type
  shall NOT appear in its `supported_events()`. For example:
  - `onScroll` does not apply to Button, Label, PictureBox
  - `onKeyDown` does not apply to PictureBox, Animator, Shape, Line
  - `onCheckedChanged` only applies to CheckBox, RadioButton
  - Non-visual controls (Timer, AgentObject, RestClient, SqlDatabase) do
    NOT receive any base events or new events from this spec — their
    existing event lists remain unchanged
- **R3.6 (ubiquitous):** Existing event names shall NOT be changed. New events
  are additive only. Where an alias is introduced (e.g. `onDoubleClick` as
  alias for `onDblClick`), BOTH names shall fire.

### R4 — Event data

- **R4.1 (ubiquitous):** Mouse events shall pass the pointer position (x, y)
  relative to the control's top-left corner.
- **R4.2 (ubiquitous):** Keyboard events shall pass the key code and modifier
  state (Shift, Ctrl, Alt, Cmd).
- **R4.3 (ubiquitous):** Drag events shall pass the drag delta (dx, dy).
- **R4.4 (ubiquitous):** Property-change events shall pass the property name
  and new value.

## 5. Acceptance criteria

- [ ] AC1 — Button supports: onClick, onDoubleClick, onRightClick,
      onMouseDown, onMouseUp, onMouseEnter, onMouseLeave, onMouseMove,
      onGotFocus, onLostFocus, onKeyDown, onKeyUp, onEnterPressed,
      onContextMenu, onHoverEnter, onHoverLeave, onResize, onResized,
      onVisibleChanged, onEnabledChanged, onLoad.
- [ ] AC2 — Right-clicking a Button fires `onRightClick` AND `onContextMenu`.
- [ ] AC3 — Resizing a Button at runtime fires `onResize` during drag, then
      `onResized` once on release.
- [ ] AC4 — TextBox supports all base events PLUS onChange, onTextChanged,
      onSelectionChanged, onKeyPress, onEnterPressed, onEscapePressed.
- [ ] AC5 — Pressing Enter in a TextBox fires `onEnterPressed`.
- [ ] AC6 — DataGrid supports onScroll, onCellDoubleClick, onColumnResize,
      onColumnResized, onSort, onSelectionChanged.
- [ ] AC7 — PictureBox fires `onImageLoaded` when an image finishes loading.
- [ ] AC8 — GroupBox fires `onScroll` when AutoScroll content is scrolled.
- [ ] AC9 — Changing a control's Visible property programmatically fires
      `onVisibleChanged`.
- [ ] AC10 — Hovering over any control for >200ms fires `onHoverEnter`;
      moving away fires `onHoverLeave`.
- [ ] AC11 — All new events are in `supported_events()` for their control types.
- [ ] AC12 — All new IDE strings are in the `Tr` table with 6 translations.
- [ ] AC13 — Existing events continue to fire exactly as before (no regressions).

## 6. Constraints & steering check

- **i18n (6 languages):** No new UI strings needed for events themselves (event
  names are English identifiers). The Events section in the properties panel
  already renders event names dynamically from `supported_events()`.
- **Generated-code / regenerate contract:** The codegen's `write_event_loop`
  generates WHEN clauses from `supported_events()`. Adding events to the
  list automatically generates the dispatch code on regenerate. No codegen
  changes needed.
- **Docs (English guide):** Update the "Events you can handle" section with
  the full event table per control type.
- **Fix vs feature:** This is a **fix** (events that should have been fireable
  from the beginning are now wired). Bump `z`.
- **Backward compatibility:** Existing event names and behavior MUST NOT change.
  New events are purely additive.

## 7. Open questions

- **Q1:** Should `onDblClick` be renamed to `onDoubleClick`? 
  **Recommendation:** Keep `onDblClick` for backward compatibility AND add
  `onDoubleClick` as an alias that fires alongside it.
- **Q2:** Should drag-and-drop events be fired at runtime only, or also in the
  designer (for control rearrangement)?
  **Recommendation:** Runtime only. Designer drag is an IDE operation, not a
  form event.
- **Q3:** How should event data (mouse position, key code, etc.) be passed to
  the COBOL event handler?
  **Recommendation:** Set well-known working-storage items before dispatching:
  `WS-EVENT-MOUSE-X`, `WS-EVENT-MOUSE-Y`, `WS-EVENT-KEY-CODE`,
  `WS-EVENT-MODIFIER-SHIFT`, etc. The codegen generates these items.
