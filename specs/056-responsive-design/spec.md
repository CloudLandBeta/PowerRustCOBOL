# Spec — Responsive design: anchoring, docking, flex/grid/flow, breakpoints and fluid type

- **Status:** draft (revision 2) — awaiting approval
- **Folder:** specs/056-responsive-design/
- **Release line:** **1.80.x** (branch `1.80.x`)
- **Author:** Claude (Fable 5.1) with the operator — revision 1 2026-09-04;
  revision 2 (Claude Opus 5.5) 2026-09-29
- **Revision 2 scope, set by the operator (2026-09-29):** develop the spec on
  the `1.80.x` line; bring **flex / grid / flow layouts, per-breakpoint
  layouts and font scaling** into scope (revision 1 excluded all three); and
  make **responsive layout the default for new projects** created with 1.80.x.

> **Standing rule for every phase of this feature — spec, plan, tasks,
> implement, docsync: DO NOT MAKE ASSUMPTIONS. READ THE CODE BEFORE
> IMPLEMENTING.** Every design statement in this spec cites the file and line
> it was read from (§9). `/plan` must re-read each cited site and cite every
> call site it touches; every task in `tasks.md` must name the code it read
> before changing it; `/implement` must verify each surface with a test that
> renders, never by inference from another surface. A claim about how the
> engine behaves that is not backed by a file:line or a test is not admissible
> in any artifact of this feature.
>
> Revision 2 re-read every citation of revision 1 on 2026-09-29. **All of them
> had moved; one (F6) had become false.** §9 carries the current lines. Line
> numbers rot within days on this codebase — `/plan` re-reads them again.

## 1. Overview

PowerRustCOBOL forms are laid out in absolute form-space pixels: every control
carries an `x, y, w, h` rectangle, the run-form window opens at the designed
size, and when the window is made larger the form stays its designed size in
the top-left corner while the rest is empty; made smaller, the form scrolls
(§9 F3). Only three things move today: a Responsive MenuBar and a StatusBar
are stretched to the window's width (§9 F21), an `AutoSize` label takes its
caption's size (§9 F6), and a container that COBOL moves carries its children
(§9 F10). Nothing else moves, stretches or reflows. This is the single largest
gap between what the platform *paints* (glass, gradients, rounded clipping,
easing) and what a current application is expected to *do* when its window,
screen or pane changes size.

This feature adds **responsive design** to a form as a form-level property —
**"Responsive design"** — under which:

1. each control follows the edges of its parent (**anchoring**) or claims an
   edge of it outright (**docking**), within **minimum and maximum sizes**;
2. a container — the form itself, a `Panel`, a `GroupBox`, a `TabControl`'s
   pages — may instead lay its children out as a **flex** row or column, a
   **grid** of tracks, or a **flow** that wraps like text;
3. the form defines **breakpoints** — named width ranges — and any control may
   carry **per-breakpoint overrides** of its layout, visibility and size;
4. text follows the surface with **font scaling**: fluid between two limits,
   stepped per breakpoint, and multiplied by the operating system's text-size
   setting where the platform exposes one.

One pure, shared layout function computes where every control lands and at
what font size for the surface it is being drawn on, and every surface — the
designer canvas, the preview, the run-form window, a form loaded into a
shell's ContentPane, and the compiled binary — draws from that same answer,
so the pixel-parity promise the platform already makes is kept rather than
bent.

**Existing forms change by nothing.** A form without the `responsive`
attribute renders exactly as it does today, on every surface. **New projects
created with 1.80.x start responsive**: their forms are created with the
switch on (§4.15).

The bar the operator set: a developer who knows React should look at the
result and want it.

## 2. Goals / Non-goals

**Goals**

- G1 — A form marked responsive resizes gracefully: controls follow the edges
  they are anchored to, stretch between opposite edges, dock to the sides of
  their parent, and fill what remains — at any window, screen or pane size.
- G2 — Zero change for existing forms. With the property off (the default for
  every form that exists today and for every form that does not carry the
  attribute), rendering is **identical**, to the pixel, on every surface.
- G3 — One layout, every surface. The designer, the preview, the run form, the
  pane occupant and the compiled binary all obtain control rectangles **and
  font sizes** from the same function with the same inputs, and a test proves
  they agree.
- G4 — The designer shows the layout **live**: drag the canvas edge and watch
  the form reflow; pick a breakpoint or a device and see the form at that size
  without changing what is designed; see a control's anchors on the control
  itself.
- G5 — Everything is deterministic and pure: the layout is a function of the
  designed form, the active breakpoint, the font factor and the available
  size; idempotent, unit-testable without a window, and it never rewrites the
  design.
- G6 — Reclaim the property names a developer expects: `Anchor` means edges
  again (it once did — §9 F5) and the canvas drag-lock that took the name gets
  its own.
- G7 — **Container layouts**: flex (row/column, wrap, grow/shrink/basis,
  justify/align, gap, order), grid (track templates with `px`, `%`, `fr`,
  `auto`, `minmax()`, `repeat()`, auto-fill; spans; gaps) and flow (the
  WinForms/PowerCOBOL-style wrapping panel), nestable inside one another and
  inside anchored or docked containers.
- G8 — **Breakpoints**: a form reshapes itself at named widths — a sidebar
  becomes a top bar, a two-column grid becomes one column, a panel hides —
  designed visually, one breakpoint at a time.
- G9 — **Fluid type**: text grows and shrinks with the surface between limits
  the developer sets, steps per breakpoint when asked, and honours the
  operator's system text size.
- G10 — **Responsive by default for new work**: every new project created with
  1.80.x creates responsive forms; nothing about an existing project changes
  unless its developer opts in.

**Non-goals** (explicitly out of scope for this spec)

- A constraint solver (Auto Layout-style inequalities between arbitrary
  controls). Anchoring, docking, flex, grid and flow are the model.
- CSS parity for its own sake. The flex and grid here are the subsets named in
  §4.12–§4.13, defined for rectangles of known size; they are not an HTML
  engine, and the Viewer's CSS engine is not rewritten by this spec (§4.12
  R49).
- Content-driven text measurement inside the layout function. Controls enter
  layout with a known size — their designed size, or the size `AutoSize`
  already computes (§9 F6); the function itself never measures text (R22).
- Per-breakpoint changes to content (captions, items, data bindings, event
  handlers). Breakpoints override layout, visibility, size and type only
  (R58).
- Any change to generated COBOL for layout. Layout is host-side. (The two new
  read-only runtime properties and the new events of §4.10 and §4.14 go
  through the existing property/event machinery.)
- Reusing the dead provisions found in the code: the legacy string `Anchor`
  values (§9 F5) are migrated, not revived as a second syntax.

## 3. User stories

- As a **form developer**, I want a form to stretch to the window the user
  gives it, so that a grid fills a large screen instead of leaving a third of
  it empty and a status bar stays at the bottom instead of floating mid-window.
- As a **form developer**, I want to say "this panel is the left sidebar, this
  toolbar sits along the top, and the grid takes whatever is left", so that I
  never compute a size by hand again.
- As a **form developer**, I want a row of buttons that spaces itself evenly,
  a card list that wraps into as many columns as fit, and a data-entry form
  laid out on a two-column grid — without writing a single `COMPUTE`.
- As a **form developer**, I want the same form to become a single column on
  a phone-width window and two columns on a desktop, designing each shape on
  the canvas rather than in code.
- As a **form developer**, I want text that is readable at 1920 px and still
  fits at 800 px, and that respects the operator who set the system text size
  larger.
- As a **form developer**, I want to design for one device size and watch the
  form at an iPhone, an iPad and a desktop width in the designer, so that I
  find layout problems before running anything.
- As a **maintainer of existing forms**, I want every form I already have to
  render exactly as it does today until I choose otherwise, so that adopting
  this feature is a decision per form, never an event that happens to me.
- As a **developer starting a new project on 1.80.x**, I want responsive
  layout on from the first form, so the modern behaviour is the one I get
  without looking for it.
- As a **COBOL programmer**, I want a control's `::Width` and `::Height` to
  tell me the size it actually has on screen, and an event when the form
  crosses a breakpoint, so that my code and the screen never disagree.
- As a **reviewer of this feature**, I want every claim about the engine in the
  plan and tasks to point at the line it was read from, so that the feature is
  built on the code that exists rather than on the code someone remembers.

## 4. Requirements (EARS)

Requirement numbers R1–R42 are revision 1's, kept stable; revised text is
marked *(rev. 2)*. R43 onward are new in revision 2.

### 4.1 The form-level switch

- **R1 (ubiquitous):** The system shall give every form a boolean property
  **`Responsive design`** (model field `responsive`, `.cfrm` attribute
  `responsive`), following the form-attribute conventions already in use
  (§9 F2).
- **R2 (ubiquitous):** The system shall treat a `.cfrm` that carries no
  `responsive` attribute as **false**, and `Form::new` shall default it to
  false, so that no existing form changes behaviour on load. *(rev. 2: the
  default for **new forms created in the IDE** is decided by the project —
  §4.15; the model and file defaults stay false.)*
- **R3 (state):** While `Responsive design` is false, the system shall render
  the form on every surface **exactly as it does before this feature** — the
  designed size inside a scrolling surface, with the Responsive MenuBar and
  StatusBar stretched as they are today (§9 F3, F21) — and a test shall prove
  the control rectangles are identical.
- **R4 (event):** When the developer toggles `Responsive design` in the
  designer, the system shall apply it live to the canvas and mark the form
  dirty, and shall not move, resize or rewrite any designed rectangle.
- **R5 (constraint):** The system shall not require any other change to a form
  for `Responsive design` to be turned on: with the switch on, every container
  at `Layout = Absolute`, every control at the default anchor (§4.2), no
  breakpoint overrides and `FontScaling = None`, a form shall render
  identically to the switch off at the designed size, and simply stop
  scrolling beyond it.

### 4.2 Anchoring

- **R6 (ubiquitous):** The system shall give every visual control an **`Anchor`**
  property whose value is a set of edges drawn from `Top`, `Bottom`, `Left`,
  `Right`, persisted as a comma-separated string (e.g. `"Top,Left,Right"`).
- **R7 (ubiquitous):** The default `Anchor` shall be `"Top,Left"`, which is by
  definition today's behaviour: a fixed offset from the top-left of the parent.
- **R8 (state):** While a form is responsive, the system shall, for each axis
  independently, position a control in an `Absolute` parent according to
  which of that axis's two edges are in its `Anchor`:
  - **leading only** (`Left` / `Top`): keep the designed offset from the
    leading edge; size unchanged;
  - **trailing only** (`Right` / `Bottom`): keep the designed offset from the
    trailing edge; size unchanged;
  - **both**: keep both offsets, so the control's size on that axis changes by
    the parent's change on that axis (stretch);
  - **neither**: keep the control's **centre** at the same fraction of the
    parent's extent (proportional), size unchanged.
- **R9 (ubiquitous):** All anchor arithmetic shall be computed against the
  **designed** rectangle and the **designed** parent size (the `.cfrm` values,
  after the active breakpoint's overrides — R58), never against the previous
  frame's result, so that the layout is idempotent and cannot drift under
  repeated resizes.
- **R10 (constraint):** The system shall never write a laid-out rectangle back
  into the design. The designed rectangles remain the source of truth; layout
  is a view of them.

### 4.3 Docking

- **R11 (ubiquitous):** The system shall give every visual control a **`Dock`**
  property with the values `None`, `Left`, `Top`, `Right`, `Bottom`, `Fill`,
  default `None`.
- **R12 (state):** While a form is responsive, the system shall lay out docked
  controls of an `Absolute` parent in **render (z-) order**, each consuming one
  edge of its parent's remaining client rectangle: `Left`/`Right` take the full
  remaining height at the control's designed width; `Top`/`Bottom` take the
  full remaining width at the control's designed height; `Fill` takes the whole
  remaining rectangle. After each docked control is placed, the remaining
  rectangle shrinks by what it took.
- **R13 (ubiquitous):** A docked control's `Anchor` shall be ignored; docking
  wins.
- **R14 (ubiquitous):** Undocked controls shall be laid out (§4.2) against the
  parent's **full** client rectangle, not the remainder after docking — so an
  anchored control and a docked sidebar do not fight, and the developer's
  designed positions mean what they show on the canvas.
- **R15 (event):** When the developer changes a control's `Dock`, the designer
  shall reflect the new layout immediately on the canvas and shall not alter
  the control's designed rectangle (which is what `Dock: None` returns to).

### 4.4 Size limits

- **R16 (ubiquitous):** The system shall give every visual control
  **`MinWidth`, `MinHeight`, `MaxWidth`, `MaxHeight`** properties, integers in
  form-space pixels, where **0 means no limit**, all defaulting to 0.
- **R17 (state):** While laying out, the system shall clamp every stretched,
  docked, grown, shrunk or grid-sized dimension to `[Min, Max]` (a non-zero
  bound applies; a zero bound does not), and shall place the clamped control
  according to its anchors or its container's alignment — a control anchored
  `Left,Right` that hits its `MaxWidth` stays attached to `Left`.
- **R18 (ubiquitous):** The system shall compute the form's **minimum size** —
  the smallest surface at which every docked control receives at least its
  minimum, no anchored control is clamped below its minimum, and every flex,
  grid and flow container can place its children at their minimums — for the
  **narrowest breakpoint's** layout, and, on a resizable run-form window,
  shall set the window's minimum inner size to it, never below 64 × 64, so the
  OS resize handle cannot produce a layout the form cannot honour. No minimum
  inner size is set anywhere today (§9 F4).

### 4.5 Containers

- **R19 (ubiquitous):** For a control whose `parent` is a container
  (`GroupBox`, `Panel`, `TabControl`), the parent rectangle used by §4.2,
  §4.3 and §4.12–§4.14 shall be that container's **client rectangle** as the
  engine already defines it — `Control::content_rect()` (§9 F7), which
  accounts for the tab strip on every `TabPosition` — computed from the
  container's **laid-out** rectangle, less the container's padding (R51).
- **R20 (ubiquitous):** Layout shall be recursive and top-down: a container is
  placed by its own parent first, then its children are placed inside it. A
  container's children shall never influence the container's own placement
  except through the form-minimum computation (R18) and a flex/grid/flow
  container sized to its content (R53).
- **R21 (ubiquitous):** Control rectangles shall remain **form-space absolute**
  in the model and in every output (§9 F8); layout produces absolute
  rectangles, not parent-relative ones, so that every existing consumer of a
  control's rectangle keeps working unchanged.

### 4.6 One layout, every surface

- **R22 (ubiquitous) *(rev. 2)*:** The system shall implement layout as **one
  pure function** in `cobolt-forms` — inputs: the designed controls, the
  designed form size, the breakpoint table and overrides, the font factor
  inputs (§4.14), the available surface size, and each control's **intrinsic
  size** (its designed size, or the size `AutoSize` measured — §9 F6); output:
  a rectangle and an effective font size per control, the active breakpoint,
  and the form minimum — with no dependency on egui, on a window, or on any
  frame state, so that it is unit-testable and the same on every surface. The
  function itself shall never measure text.
- **R23 (ubiquitous) *(rev. 2)*:** Every surface that draws a form into a
  rectangle shall obtain control rectangles and font sizes from that function
  with the surface's own available size: the **run-form window** (its inner
  size), a **form loaded into a shell ContentPane** (the pane rectangle below
  the breadcrumb band), the **root form of a shell** (its ContentPane plus the
  rail's designed column — the size the host already reports since 1.70.343,
  §9 F24), the **preview**, the **designer canvas**, and the **compiled
  binary**, which shares the run-form host (§9 F9). The plan shall list each
  call site by reading it; the tasks shall test each one.
- **R24 (ubiquitous):** The function shall be **idempotent**: applying it to
  its own output at the same available size yields the same output; and
  **pure with respect to size**: the same inputs always yield the same
  rectangles regardless of what was rendered before.
- **R25 (event):** When a responsive form's available size changes at run
  time, the system shall recompute the layout **in the same frame** the new
  size is observed, with no intermediate frame drawn at the old layout.

### 4.7 Order of precedence with what already moves controls *(rev. 2)*

The engine already has **six** mechanisms that rewrite control rectangles at
render time (§9 F6, F10, F21) — revision 1 knew three. This spec does not
change them; it fixes their order.

- **R26 (ubiquitous) *(rev. 2)*:** For a responsive form the order shall be:
  (1) resolve the active breakpoint and apply its overrides to the designed
  values (§4.13); (2) compute the font factor and effective font sizes
  (§4.14); (3) measure `AutoSize` controls at their **effective** font size
  (§9 F6); (4) run the responsive layout (§4.2–§4.5, §4.12); then (5) the
  existing mechanisms — COBOL-moved container offsets, Splitter pane reflow,
  SideMenu rail narrowing and content slide, repeating-group instancing —
  operate on **those** rectangles exactly as they operate on designed
  rectangles today.
- **R27 (ubiquitous):** Controls whose position is **owned by another
  mechanism** — a Splitter pane, a SideMenu footer, a repeating-group instance
  (§9 F11) — shall not be anchored, docked or placed by a flex/grid/flow
  parent: their layout properties shall be ignored and their rows hidden in the
  inspector, with the owner control itself (the Splitter, the SideMenu, the
  template group) remaining anchorable, dockable and placeable as a whole.
- **R28 (ubiquitous):** Animations that offset a control (`slide_dx`,
  `slide_dy`, §9 F12) shall apply on top of the laid-out rectangle, as they
  apply on top of the designed one today.
- **R43 (state):** While a form is responsive, `stretch_window_bars` (§9 F21)
  shall **not** run on it: a Responsive MenuBar and a StatusBar are laid out
  like any other control, and their default `Anchor` on a responsive form
  shall be `Top,Left,Right` (MenuBar) and `Bottom,Left,Right` (StatusBar), so a
  responsive form keeps today's visible behaviour for them without a second
  mechanism doing the same job.

### 4.8 The designer

- **R29 (state):** While a form is responsive, the designer canvas shall show
  the form **laid out at the current canvas size**, and dragging the canvas
  resize grip shall reflow the controls live, while the designed rectangles —
  what is saved and what the property rows show — stay exactly what the
  developer set.
- **R30 (ubiquitous) *(rev. 2)*:** The designer shall offer a **"View at"**
  selector drawing on the existing target presets (§9 F13) **and on the form's
  breakpoints** (§4.13), which sets the canvas's available size for viewing
  only and never changes the form's designed `Width`/`Height` or its `Target`.
- **R31 (ubiquitous):** When a control is selected on a responsive form in an
  `Absolute` parent, the designer shall draw an **anchor gizmo** on the control
  — one pin per edge, lit when that edge is in the control's `Anchor` — and
  clicking a pin shall toggle that edge. A docked control shall show its dock
  edge instead of pins.
- **R32 (ubiquitous) *(rev. 2)*:** The properties pane shall show a **Layout**
  section — for the form and containers: `Layout` and the container properties
  of §4.12; for every control that is not owner-positioned (R27): `Anchor`
  (four checkboxes) and `Dock` when its parent is `Absolute`, the item
  properties of its parent's layout (§4.12) otherwise, and the four size
  limits — **only when the form is responsive**, with a one-line hint where the
  section would be otherwise saying that `Responsive design` is off.
- **R33 (ubiquitous):** Moving or resizing a control on a responsive canvas
  shall edit its **designed** rectangle by the inverse of the current layout
  mapping, so that what the developer drags is what they see — never the raw
  designed value silently offset from the cursor.
- **R44 (event):** When the developer drags a child **within** a flex, grid or
  flow container, the designer shall show the drop position as an insertion
  marker between items (flex/flow) or as the target cell (grid), and dropping
  shall change the child's `Order` (flex/flow) or its `GridColumn`/`GridRow`
  (grid) — never its designed `x`/`y`, which such a parent does not use.
- **R45 (state):** While a flex, grid or flow container is selected, the
  designer shall overlay its structure — gaps and item boundaries (flex/flow),
  track lines with their sizes (grid) — so the developer sees the model, not
  only the result.

### 4.9 Reclaiming the names

- **R34 (ubiquitous):** The canvas drag-lock that is currently stored as the
  boolean `Anchor` (§9 F5) shall become a boolean **`Locked`** property, with
  its own inspector label and KB entry, and `Anchor` shall carry edges only.
- **R35 (event):** When a `.cfrm` is loaded that carries a **boolean** `Anchor`,
  the system shall migrate it: `true` → `Locked = true`, `false` →
  `Locked = false`, and `Anchor` → `"Top,Left"`; a `.cfrm` that carries the
  legacy **string** `Anchor` (such as `"Top,Left"`, present in real forms —
  §9 F5) shall keep it as a valid edge set. Both migrations shall happen through
  the **same load-time seeding path both `Control::new` and `load_form` read**
  (§9 F14), so the two boundaries cannot drift, and each shall be pinned by a
  test that loads XML rather than constructing a control.
- **R36 (constraint):** The system shall not confuse `Anchor` with the
  Snackbar's `StackAnchor` (§9 F15), which is a nine-position placement of a
  different kind and is unchanged by this spec.

### 4.10 Runtime and COBOL

- **R37 (ubiquitous) *(rev. 2)*:** A runtime read of a control's geometry
  properties from COBOL (`X`, `Y`, `Width`, `Height` through the `::` surface)
  on a responsive form shall return the **laid-out** values for the surface the
  form is currently on. Today every such read returns the designed value
  seeded at start-up or the program's own last write — the host never sends a
  control's live rectangle back (§9 F22) — so the host shall mirror the
  laid-out rectangles onto the control objects, after each layout that changed
  them, the way it already mirrors the form's own `Width`/`Height` (§9 F24).
  (`Left`/`Top` are not properties today — §9 F22 — and this spec does not add
  them.)
- **R38 (event):** When COBOL writes a geometry property on a responsive form,
  the system shall treat the write as a change to the **designed** rectangle
  and re-run the layout, so that the write composes with anchoring rather than
  being overwritten by the next resize.
- **R39 (event) *(rev. 2)*:** When a responsive form's available size changes
  at run time, the system shall fire the form-level **`onResize`** event —
  which the host already raises for window and shell forms, with the form's
  `Width`/`Height` mirrored first (§9 F24) — **after** the layout has been
  applied and mirrored (R37), so a handler observing geometry sees the new
  values.
- **R46 (ubiquitous):** The system shall expose two read-only form properties:
  **`Breakpoint`** — the name of the active breakpoint (§4.13) — and
  **`FontScale`** — the effective font factor as a decimal (§4.14).
- **R47 (event):** When the active breakpoint changes at run time, the system
  shall fire the form-level event **`onBreakpointChanged`** after the new
  layout is applied and mirrored, and before `onResize` for the same size
  change. It shall not fire for the breakpoint the form opens in.
- **R48 (constraint):** A form handler that lays controls out by hand (the
  pattern `onResize` handlers use today) shall keep working on a responsive
  form: a COBOL geometry write is a designed-value write (R38), so the
  developer's code and the layout compose rather than fight.

### 4.11 Process constraints (carried into every phase)

- **R40 (constraint):** No artifact of this feature — plan, task, code,
  commit message, doc — shall assert how existing code behaves without citing
  the file and line that was read, or a test that demonstrates it.
- **R41 (constraint):** `/plan` shall re-read every site cited in §9 and shall
  enumerate, by reading, every place a control's rectangle **and every place a
  control's font size** is consumed on each surface, before choosing where the
  layout function is called.
- **R42 (constraint):** `/implement` shall verify each surface (R23) with a
  test that renders headlessly and reads `RenderOutput.control_rects`
  (§9 F16), never by reasoning that "the surfaces share code".

### 4.12 Container layouts: Flex, Grid and Flow

- **R49 (ubiquitous):** The system shall give the **form** and every container
  (`Panel`, `GroupBox`, `TabControl` — whose pages all use the TabControl's
  layout) a **`Layout`** property with the values **`Absolute`** (default —
  §4.2 anchoring and §4.3 docking), **`Flex`**, **`Grid`** and **`Flow`**. The
  solver shall be new, pure code (R22). The Viewer's CSS engine lays out flex
  and grid by painting (§9 F25) and cannot be called on control rectangles;
  its egui-free value types (`BoxLayout`, `LayoutKind`, `GridTrack`,
  `CrossAlign`, `MainAlign`) and its track parser may be moved to a shared
  module and reused (§9 F25), and the Viewer shall keep its behaviour exactly.
- **R50 (ubiquitous):** Children of a `Flex`, `Grid` or `Flow` container shall
  be placed by the container: their `Anchor` and `Dock` are ignored, and their
  designed `x`/`y` are used only to derive a default order (R54). Their
  designed width and height are their **intrinsic size** (or the `AutoSize`
  size — R22).
- **R51 (ubiquitous):** Every container shall have a **`Padding`** (uniform, in
  form pixels; `PaddingLeft/Top/Right/Bottom` override one side), default 0,
  applied inside the client rectangle for every `Layout` value. `/plan` shall
  read whether a `Padding` property is already seeded on containers (it
  appears in saved Shape controls — §9 F26) and reuse it if so.
- **R52 (ubiquitous) — Flex container properties:** `FlexDirection`
  (`Row` default, `Column`, `RowReverse`, `ColumnReverse`); `FlexWrap`
  (`NoWrap` default, `Wrap`, `WrapReverse`); `JustifyContent` (`Start` default,
  `Center`, `End`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly`);
  `AlignItems` (`Stretch` default, `Start`, `Center`, `End`); `AlignContent`
  for wrapped lines (`Stretch` default, `Start`, `Center`, `End`,
  `SpaceBetween`, `SpaceAround`); `Gap`, `RowGap`, `ColumnGap` (form pixels).
- **R53 (ubiquitous) — Flex item properties:** `FlexGrow` (decimal ≥ 0,
  default 0); `FlexShrink` (decimal ≥ 0, default 1); `FlexBasis` (`Auto`
  default = the intrinsic size on the main axis, or form pixels, or a
  percentage of the container's main size); `AlignSelf` (`Auto` default, or
  an `AlignItems` value); `Order` (integer, default 0). The algorithm shall be
  the CSS flexbox algorithm restricted to these properties **for items of
  known size**: free space distributed to growing items in proportion to
  `FlexGrow`; overflow taken from shrinking items in proportion to
  `FlexShrink × basis`; each result clamped to the item's `Min`/`Max` (R17)
  with the standard re-distribution loop; lines broken greedily when wrapping;
  `JustifyContent` applied to what remains after growth. (The Viewer's shrink
  is a heuristic and has no `order` or reverse directions — §9 F25 — so its
  behaviour is **not** the reference; CSS is.) A container whose size on an
  axis is not constrained by its own parent (for example a `Column` flex
  child of a scrolling form) shall size to its content on that axis.
- **R54 (ubiquitous):** The order of a flex or flow container's items shall be
  by `Order`, then by the designed position in reading order (top-to-bottom,
  then left-to-right; right-to-left for `RowReverse`) — so the order the
  developer sees on the canvas when first switching a container to `Flex` is
  the order it keeps.
- **R55 (ubiquitous) — Grid container properties:** `GridColumns` and
  `GridRows`, each a track list whose tokens are `Npx`, `N%`, `Nfr`, `Auto`
  (the largest intrinsic size among the items that sit only in that track),
  `MinMax(a, b)`, `Repeat(N, tracks)` and `Repeat(AutoFill, MinMax(min, b))`
  (as many columns of at least `min` as fit); an empty `GridRows` makes rows
  implicit, each `Auto`; `Gap`, `RowGap`, `ColumnGap`; `JustifyItems` and
  `AlignItems` (`Stretch` default, `Start`, `Center`, `End`). Fixed and
  percentage tracks are sized first, `Auto` tracks next, and `fr` tracks share
  what remains, each clamped by its `MinMax`.
- **R56 (ubiquitous) — Grid item properties:** `GridColumn` and `GridRow`
  (1-based; 0 = auto-placed, the default); `ColumnSpan` and `RowSpan` (default
  1); `JustifySelf` and `AlignSelf` (`Auto` default). Auto-placement shall be
  row-major, in item order (R54), skipping occupied cells, never back-filling
  (no "dense" packing). An item whose explicit cell lies outside the defined
  tracks shall extend the implicit grid, never be dropped.
- **R57 (ubiquitous) — Flow:** `Flow` shall behave as the wrapping panel
  PowerCOBOL and WinForms developers know: items keep their intrinsic size, run
  in `FlowDirection` (`LeftToRight` default, `TopDown`, `RightToLeft`,
  `BottomUp`), wrap when `WrapContents` (default true) and the next item does
  not fit, separated by `Gap`; an item with `FlowBreak = true` ends the line
  after itself. It is defined as a flex container with grow 0 and shrink 0, and
  shall share the flex solver.

### 4.13 Breakpoints

- **R58 (ubiquitous):** A responsive form shall carry a **breakpoint table**:
  an ordered list of named ranges by **available width** in form pixels. The
  default table for a new form shall be **`Compact`** (< 600), **`Medium`**
  (600–1023) and **`Expanded`** (≥ 1024). The developer may rename, add or
  remove breakpoints and move their thresholds; a project may carry a default
  table for its new forms (§4.15).
- **R59 (ubiquitous):** The **base design** — the `.cfrm` values without
  overrides — is the form as designed at its own `Width`; the breakpoint that
  contains the designed `Width` is the **design breakpoint** and carries no
  overrides. Every other breakpoint may carry **overrides**: sparse
  `(control, property, value)` entries replacing the base value while that
  breakpoint is active. Overrides of different breakpoints shall not cascade:
  the active layout is always base + the active breakpoint's own overrides.
- **R60 (ubiquitous):** The properties a breakpoint may override shall be
  exactly: `Visible`; the designed `X`, `Y`, `Width`, `Height`; `Anchor`,
  `Dock`, the four size limits; `Layout`, every container property of §4.12
  and `Padding`; every item property of §4.12; `FontSize`; and, for the form,
  `FontScaling` factors (§4.14). Content — captions, text, items, bindings,
  handlers — shall not be overridable.
- **R61 (state):** While laying out, the system shall pick the active
  breakpoint by the **available width of the form's surface** (R23) on every
  layout pass. (The Viewer's `@media` rules compare against a fixed 1024 px
  once, at parse time — §9 F25 — which is exactly the behaviour this rule
  forbids.)
- **R62 (ubiquitous):** A control hidden by a breakpoint override shall be laid
  out as absent: it takes no dock edge, no flex/flow slot and no grid cell,
  and its children are hidden with it. It keeps its state and its COBOL
  object; a COBOL write to its `Visible` wins over the override (R64).
- **R63 (ubiquitous):** The `.cfrm` shall store the table and the overrides
  inside `<Form>` as `<Breakpoints>` with one `<Breakpoint name=… min-width=…>`
  per entry, each holding `<Override control=… property=…>value</Override>`
  elements; a form with no overrides and the default table shall write
  nothing, so it stays byte-identical to revision-1 output.
- **R64 (ubiquitous):** Precedence of a property's value shall be: a COBOL
  runtime write, else the active breakpoint's override, else the base design.
- **R65 (event):** In the designer, when a breakpoint other than the design
  breakpoint is selected (R30), every edit to an overridable property shall be
  recorded as an **override for that breakpoint**, shown with a marker on the
  property row and a "reset to base" action; edits to non-overridable
  properties shall edit the base design and say so.

### 4.14 Font scaling

- **R66 (ubiquitous):** A form shall carry **`FontScaling`** with the values
  **`None`** (default — today's sizes), **`Fluid`** and **`Stepped`**, and
  the limits **`MinFontScale`** (default 0.85) and **`MaxFontScale`** (default
  1.50).
- **R67 (state):** While `FontScaling = Fluid`, the form factor shall be
  `available width ÷ designed width`, clamped to `[MinFontScale,
  MaxFontScale]`; while `Stepped`, it shall be the **factor of the active
  breakpoint** (each breakpoint carries one, default 1.0).
- **R68 (ubiquitous):** Where the platform exposes a system text-size setting,
  the system shall multiply the form factor by it (the **system text factor**;
  1.0 where there is none). Nothing reads such a setting today (§9 F27);
  `/plan` shall read, per platform, where it can be obtained, and say where it
  cannot.
- **R69 (ubiquitous):** A control's **effective font size** shall be its
  `FontSize` (after overrides) × the form factor × the system text factor,
  clamped to the control's **`MinFontSize`**/**`MaxFontSize`** (0 = no limit,
  default 0) and to the engine's existing bounds; a control with
  **`ScaleFont = false`** (default true) keeps its unscaled `FontSize`.
- **R70 (ubiquitous):** Every paint site that reads a control's `FontSize`
  shall go through **one resolver** that applies R69 and accepts decimal
  values. Today most sites use `ctrl_font_size`, but several parse `FontSize`
  themselves with their own defaults and bounds, and the shared parse drops a
  decimal or padded value to the 4 pt floor (§9 F27); `/plan` shall enumerate
  every such site by reading and route each through the resolver, and a test
  shall prove none is left.
- **R71 (ubiquitous):** A COBOL read of `::FontSize` shall return the
  **designed** (unscaled) size the program works in, and a write shall set the
  designed size; the form's `FontScale` (R46) reports the factor.
- **R72 (constraint):** Font scaling shall never change a control's designed
  rectangle. It changes layout only through `AutoSize` measurement (R26 step 3)
  and through overrides the developer made.

### 4.15 Responsive by default for new projects (1.80.x)

- **R73 (ubiquitous):** A project manifest shall carry a `[forms]` key
  **`responsive`** (boolean). Its serde default shall be **false**, so every
  project that exists before 1.80.x — which lacks the key — keeps creating
  non-responsive forms. This follows the split the project model already makes
  between `FormsConfig::new_project_defaults()` and `impl Default for
  FormsConfig` (§9 F28). The compiler's own copy of the manifest types shall
  gain the same key (§9 F28).
- **R74 (event):** When a **new project** is created with 1.80.x, the system
  shall write `responsive = true` into its `[forms]` section (through
  `new_project_defaults()`), and the project's default breakpoint table
  (R58).
- **R75 (event):** When a **new form** is created in a project whose
  `[forms] responsive` is true, the system shall create it with
  `responsive = true` **written into the `.cfrm`** — not resolved at run time
  from the project — so the form means the same thing wherever it is copied,
  pasted or built. A form created in a project without the key is created
  non-responsive, exactly as today.
- **R76 (ubiquitous):** The project settings shall show the `[forms]
  responsive` choice as **"New forms are responsive"**, so a developer of an
  older project can opt in for future forms; changing it shall never modify an
  existing form.
- **R77 (constraint):** No project upgrade shall turn existing forms
  responsive. The project-upgrade mechanism (§9 F29) is keyed on the manifest's
  `structure`, not on the app version, and this feature adds **no** upgrade
  step: adopting responsive layout for an existing form is always the
  developer's per-form decision (G2). The manifest's `built_with_version`
  records the last full build, not the project's origin (§9 F28), and shall
  not be used to infer anything here.

### 4.16 Validation against the example projects (operator's golden rule)

> **Operator, 2026-09-29: the golden rule is not to break existing code. Every
> change to the graphics engine made for this feature is validated against the
> PowerDemo3 and PowerChat examples.** They are the product's two real
> applications — 48 and 14 forms (`examples/PowerDemo3/forms`,
> `examples/PowerChat/forms`, `.bak` files excluded) — and they exercise every
> mechanism of §4.7: SideMenu shells, Splitters, repeating groups, AutoSize,
> Viewers, charts, TabControls, child windows and COBOL-driven geometry.

- **R78 (constraint):** No change to `cobolt-forms` (engine, paint, render,
  layout), `cobolt-form-host` or the designer canvas shall be merged on the
  1.80.x line unless the **example corpus** — every `.cfrm` of PowerDemo3 and
  PowerChat — passes the checks of R79–R81 in the same change.
- **R79 (constraint) — pixel parity of what exists:** a **golden** of the
  corpus shall be captured on the line's base commit **before the first engine
  change**: for every form, headless renders at three surface sizes (smaller
  than, equal to and larger than designed) recording `control_rects`, the
  effective font size of every control, and a shape-level digest of the frame.
  Every later change shall reproduce the golden **exactly** for the corpus as
  it is (no form responsive). A difference fails the change; a golden is
  re-captured only when the operator accepts a named, intended visual change.
- **R80 (constraint) — the programs still run:** the example tests that build
  and run these projects shall stay green on every change —
  `powerchat_compiles`, `powerchat_runs`, `props_demo_runs`, the
  `*_demo_compiles` tests of `cobolt-ide`, and the PowerDemo3 form tests of
  `cobolt-forms` and `cobolt-runtime` (`/plan` lists them all by reading
  `crates/*/tests`); the forms' generated COBOL shall be byte-identical before
  and after an engine change.
- **R81 (constraint) — responsive on, nothing moves:** for every form of the
  corpus, a copy with only `responsive="true"` added (default anchors,
  `Absolute` containers, no overrides, `FontScaling = None`) shall render at its
  designed size exactly as the golden does (R5), in every surface of R23.
- **R82 (ubiquitous):** Each engine change shall report, in its test output,
  the corpus it checked — forms, sizes, rectangles compared, differences
  (GOLDEN RULE #7: quantified, human-readable results).

## 5. Acceptance criteria

Each criterion is a test unless marked *(manual)*; the tasks phase turns them
into named tests. AC1–AC22 are revision 1's; revised ones are marked.

- [ ] **AC1 (R1, R2)** — A `.cfrm` without `responsive` loads with
  `responsive == false`; one with `responsive="true"` loads true; saving writes
  the attribute only when true (so untouched forms stay byte-identical on
  save).
- [ ] **AC2 (R3, G2, R79)** — For **every form of PowerDemo3 and PowerChat**
  and every fixture form in the test corpus, rendering headlessly at three
  surface sizes (smaller, equal, larger than designed) with
  `responsive == false` yields `control_rects`, font sizes and frame digest
  identical to the pre-feature engine (a golden captured before the change).
  Zero drift permitted. **This is the first test written.**
- [ ] **AC3 (R5)** — A responsive form whose containers are `Absolute`, whose
  controls carry the default `Anchor`, with no overrides and `FontScaling =
  None`, renders identically to the non-responsive form at the designed size.
- [ ] **AC4 (R8)** — Pure layout unit tests cover all sixteen anchor
  combinations of one control at a larger and a smaller surface: fixed
  leading, fixed trailing, stretch, proportional, per axis.
- [ ] **AC5 (R9, R24)** — `layout(layout(f, s), s) == layout(f, s)` for every
  fixture; and `layout(f, s2)` after `layout(f, s1)` equals `layout(f, s2)`
  computed cold.
- [ ] **AC6 (R12, R13, R14)** — Dock order: three controls docked
  `Top`, `Left`, `Fill` in that z-order produce the expected rectangles; reorder
  to `Left`, `Top`, `Fill` produces the other expected rectangles; an anchored
  control in the same form is positioned against the full client rect.
- [ ] **AC7 (R16, R17)** — Stretch, dock, grow and grid respect Min/Max: a
  `Left,Right` control with `MaxWidth` stays attached to `Left` at max; a
  `Fill` control with `MinHeight` is never shorter than it; a growing flex item
  at its `MaxWidth` hands the rest to its siblings.
- [ ] **AC8 (R18) *(rev. 2)*** — The computed form minimum equals the
  hand-derived value for fixtures with docked, anchored, flex and grid
  controls, at the narrowest breakpoint; the run-form window builder receives
  it as its minimum inner size.
- [ ] **AC9 (R19, R20, R21)** — Children of a `TabControl` on each of the four
  `TabPosition`s, of a `GroupBox`, and of a `Panel` lay out inside the
  container's `content_rect()` computed from the container's laid-out rect,
  less `Padding`; every output rect is form-space absolute.
- [ ] **AC10 (R23, R42) *(rev. 2)*** — One test per surface — run-form window,
  shell root form, pane occupant, preview, designer canvas — renders the same
  responsive fixture (anchors, a dock, a flex row, a grid, an override and
  fluid type) at the same available size and asserts identical `control_rects`
  and identical effective font sizes. The compiled-binary path is covered by
  the run-form host test (§9 F9) plus a build-and-run smoke test.
- [ ] **AC11 (R26) *(rev. 2)*** — A responsive form containing a Splitter, a
  collapsed SideMenu, a repeating group, an `AutoSize` label under `Fluid`
  type and a container moved by COBOL renders with each mechanism operating in
  the order of R26: the label is measured at its effective size before layout;
  the others operate on the laid-out rects.
- [ ] **AC12 (R27)** — A Splitter pane, a SideMenu footer and a repeating
  instance with `Anchor`/`Dock`/flex item properties set are laid out by their
  owner, unchanged, and their inspector rows are hidden.
- [ ] **AC13 (R29, R33)** — Designer test: on a responsive canvas at a larger
  size, dragging a `Right`-anchored control by +10 px changes its **designed**
  `x` by exactly +10 px.
- [ ] **AC14 (R30) *(rev. 2)*** — Selecting a "View at" preset or a breakpoint
  changes the canvas available size and leaves `form.width`, `form.height` and
  `form.target` untouched.
- [ ] **AC15 (R31)** *(manual + shape-dump)* — The anchor gizmo is drawn only on
  responsive forms; a shape dump shows four pins on a selected control with the
  lit ones matching `Anchor`; clicking a pin toggles the edge (unit test on the
  hit-test function).
- [ ] **AC16 (R34, R35)** — Loading XML with `<Property name="Anchor">true</Property>`
  yields `Locked == true` and `Anchor == "Top,Left"`; with `false` yields
  `Locked == false`; with `Top,Left` keeps it; the designer's drag-lock reads
  `Locked`; and `Control::new` seeds `Locked = false`, `Anchor = "Top,Left"`,
  `Dock = "None"`, the four limits `0` — with the drift test over
  `ControlType::ALL` extended to cover them.
- [ ] **AC17 (R36)** — A Snackbar with `StackAnchor = BottomCenter` and
  `Anchor = "Top,Left"` keeps both, independently.
- [ ] **AC18 (R37, R38, R39) *(rev. 2)*** — A runtime program on a responsive
  form reads `::Width` after a resize and gets the laid-out width; writes
  `::Width` and the next layout composes with it; `onResize` fires once per
  size change, after layout and after the mirror.
- [ ] **AC19 (i18n) *(rev. 2)*** — Every new label (`Responsive design`,
  `Layout`, `Anchor`, `Dock`, `Locked`, `View at`, the four limits, the "off"
  hint, every §4.12 property and value, `Breakpoint`, the breakpoint editor,
  "reset to base", `FontScaling` and its limits, `ScaleFont`,
  `MinFontSize`/`MaxFontSize`, "New forms are responsive") exists in all six
  languages and the i18n completeness test is green.
- [ ] **AC20 (KB) *(rev. 2)*** — The KB property tables carry every new control
  property, the new form properties (`Responsive design`, `Layout` and its
  container properties, the breakpoint table, `FontScaling`, `MinFontScale`,
  `MaxFontScale`, the read-only `Breakpoint` and `FontScale`) and the
  `onBreakpointChanged` event; the chunked store is rebuilt;
  `prebuilt_chunked_kb_matches_the_published_documentation` is green. (There is
  no form-level property table today — §9 F18 — so one is added.)
- [ ] **AC21 (docs) *(rev. 2)*** — `docs/developers-guide-en.md` gains a
  chapter "Responsive design" (anchoring and docking, flex, grid, flow,
  breakpoints, font scaling, the new-project default) written for a PowerCOBOL
  / isCOBOL developer, with mermaid diagrams where they fit; the support matrix
  gains rows under `PRC`; screenshot slots are left for `/doc-shots`.
- [ ] **AC22 (R40, R41)** — `plan.md` cites a file:line for every existing-code
  claim, and lists every rectangle consumer and every font-size consumer per
  surface; `tasks.md` names, for each task, the code read before the change.
  *(Checked in `/analyze`.)*
- [ ] **AC23 (R43)** — On a responsive form, a Responsive MenuBar and a
  StatusBar are placed by the layout (default anchors per R43) and
  `stretch_window_bars` is not called; on a non-responsive form it still is.
- [ ] **AC24 (R52, R53)** — Pure flex tests: every `FlexDirection` ×
  `JustifyContent` × `AlignItems` at a larger and a smaller container; grow
  ratios 1:2; shrink weighted by basis; wrap into two lines with
  `AlignContent`; `Order` and reverse directions; a content-sized column.
  Each expected rectangle is derived by hand in the test's comment.
- [ ] **AC25 (R55, R56)** — Pure grid tests: `200px 1fr 2fr`; `Repeat(3, 1fr)`;
  `Repeat(AutoFill, MinMax(160px, 1fr))` at three widths (1, 3 and 5 columns);
  `Auto` tracks; spans; explicit cells beyond the template extending the
  implicit grid; auto-placement skipping an explicitly placed item.
- [ ] **AC26 (R57)** — Flow: items of different widths wrap at the container
  edge in each `FlowDirection`; `FlowBreak` forces a line; `WrapContents =
  false` keeps one line.
- [ ] **AC27 (R49)** — The Viewer's layout tests pass unchanged after any
  shared types move (Viewer behaviour is not altered).
- [ ] **AC28 (R58–R62, R64)** — A fixture with `Compact`/`Medium`/`Expanded`
  overrides (a sidebar `Dock: Left` becoming `Dock: Top`, a grid of 2 columns
  becoming 1, a panel hidden at `Compact`) lays out as expected at 480, 800 and
  1280 px; a hidden control takes no slot; a COBOL `Visible` write beats the
  override; the active breakpoint changes exactly at the thresholds.
- [ ] **AC29 (R63)** — `<Breakpoints>` round-trips; a form with the default
  table and no overrides saves byte-identical to one written before revision 2.
- [ ] **AC30 (R65)** — Designer test: with `Compact` selected, changing a
  control's `Width` records a `Compact` override and leaves the base `Width`
  unchanged; "reset to base" removes it; changing `Caption` edits the base.
- [ ] **AC31 (R46, R47)** — A runtime program reads `me::Breakpoint` and
  `me::FontScale`; `onBreakpointChanged` fires once when a resize crosses a
  threshold, after the mirror and before `onResize`, and not at start-up.
- [ ] **AC32 (R66–R69, R71, R72)** — `Fluid` at 0.5×, 1× and 3× the designed
  width yields factors clamped to 0.85, 1.0 and 1.5; `Stepped` uses the active
  breakpoint's factor; `MinFontSize`/`MaxFontSize` and `ScaleFont = false` are
  honoured; a COBOL read of `::FontSize` returns the designed size; no designed
  rectangle changes.
- [ ] **AC33 (R70)** — A test walks every control type, sets a decimal
  `FontSize` and a factor, renders headlessly and asserts every text-painting
  site used the resolver's size (a shape-level check of galley font sizes).
- [ ] **AC34 (R68)** — With a system text factor injected at 1.25, effective
  sizes multiply by it; the per-platform source `/plan` names is covered where
  the platform has one, and documented where it has none.
- [ ] **AC35 (R73–R77)** — A new project created by the IDE's own creation path
  carries `[forms] responsive = true` and the default breakpoint table; a form
  created in it saves `responsive="true"`; a manifest without the key loads
  false and its new forms are non-responsive; toggling "New forms are
  responsive" modifies no existing `.cfrm`; no project upgrade is offered for
  this feature; the compiler's manifest copy reads the key.

- [ ] **AC36 (R78, R79)** — The golden of the 62 example forms exists on the
  line before any engine change (its commit precedes the first engine commit)
  and a test compares against it; `/analyze` checks the order.
- [ ] **AC37 (R80)** — The example build/run tests listed in `plan.md` are green
  on every commit that touches the engine, and the generated `.cbl` of both
  projects is byte-identical to the base commit's.
- [ ] **AC38 (R81)** — For each of the 62 forms, the `responsive="true"` copy at
  its designed size equals the golden, on every surface of R23.
- [ ] **AC39 (R82)** — The corpus test prints a summary block: forms checked,
  sizes, rectangles and font sizes compared, differences (0 expected), timing.

## 6. Constraints & steering check

- **i18n (6 languages):** new `Tr` fields for every label in AC19 —
  EN/ES/PT/JA/ZH/FR, in `crates/cobolt-ide/src/i18n.rs` (§9 F17). No literals.
- **Generated COBOL / regenerate contract:** unaffected by layout. The two
  read-only form properties and `onBreakpointChanged` use the existing
  property and event machinery; `/plan` shall confirm by reading whether the
  event catalogue (`FORM_EVENT_GROUPS`) and the compiler's event tables need
  entries (they do for every other form event).
- **System KB:** required in the same change (tech.md hard constraint) —
  property tables in `cobolt-compiler`'s doc tables (§9 F18), plus the chunked
  store rebuild (`cargo run -p cobolt-ide --example build_chunked_kb`) and its
  committed `assets/knowledge/chunked.data`.
- **Docs:** English guide chapter + support-matrix rows (AC21). 1.80.x is a
  **minor** release: under GOLDEN RULE #8 the operator's raising of `y` runs
  the translation regeneration cycle for every document changed on the line —
  at release, not per change.
- **Fix vs feature:** **feature** — new user-visible functionality, on the
  **`1.80.x`** branch (operator, 2026-09-29), local until the operator allows it
  on the remote. The version on the line is `1.80.z`.
- **Verify-first:** every test reports what it measured; no acceptance
  criterion is ticked from a filtered grep (see the test-sweep rules in the
  project memory).
- **Pixel parity (product promise):** R23/AC10 are the guard.
- **Don't break existing code (operator's golden rule):** §4.16 — every engine
  change is validated against the PowerDemo3 and PowerChat forms (golden
  renders, their build/run tests, byte-identical generated COBOL).
- **Rust only:** the layout function and all tests are Rust; no scripts in the
  tree.
- **No self-resizing windows (GOLDEN RULE):** R18 sets a *minimum* inner size
  only; nothing in this feature resizes a window on its own. The shell's rail
  toggle already resizes its window by design (§9 F24) and is unchanged.

## 7. Open questions

Revision 1's Q1–Q6 were resolved by the operator's delegation ("I will rely on
your decisions as long as they are the best in the long run"); revision 2's
Q7–Q14 are decided the same way and listed so each choice is visible and
reversible at approval.

- **Q1 — Reclaim `Anchor` for edges and move the drag-lock to `Locked`?**
  Decision: reclaim (R34/R35).
- **Q2 — Proportional anchoring (neither edge)?** Decision: include (R8).
- **Q3 — Docked thickness: designed size or a separate `DockSize`?** Decision:
  designed size.
- **Q4 — A non-responsive form's Layout section: greyed or hidden?** Decision:
  hidden with a one-line hint (R32).
- **Q5 — `onResize`: form-level only, or per control?** Decision: form-level
  (R39); it exists since 1.70.341 and this spec orders it after layout.
- **Q6 — The window minimum: derived from the layout, or the designed size?**
  Decision: derived (R18), at the narrowest breakpoint.
- **Q7 — Flex/grid semantics: CSS or our own?** Decision: CSS flexbox and grid
  semantics restricted to the listed properties, for items of known size (R53,
  R55). A React developer's intuition then transfers intact; inventing our own
  would make every tutorial on the web wrong for us.
- **Q8 — Flow: a separate mode or just "flex with wrap"?** Decision: separate
  `Layout = Flow` with WinForms/PowerCOBOL vocabulary (`FlowDirection`,
  `WrapContents`, `FlowBreak`), implemented on the flex solver (R57). The
  audience of this product knows `FlowLayoutPanel`, not `flex-wrap`.
- **Q9 — Breakpoint cascade: mobile-first inheritance or base + own
  overrides?** Decision: base + the active breakpoint's own overrides, no
  cascade (R59). Predictable in the designer — what a breakpoint shows is its
  overrides and nothing inherited from a third breakpoint.
- **Q10 — Which width selects the breakpoint: window, screen or surface?**
  Decision: the form's surface (R61) — so a form in a shell's ContentPane
  responds to the pane, and a form docked beside a collapsed rail responds to
  what it really has.
- **Q11 — Default breakpoints.** Decision: `Compact` < 600 ≤ `Medium` < 1024 ≤
  `Expanded` (R58) — the widely used window-size-class split, three steps
  (phone, tablet/narrow window, desktop), editable per form and per project.
- **Q12 — Fluid type default for new forms?** Decision: `FontScaling = None`
  even in new projects. Responsive *layout* is the new default (§4.15); scaling
  type is a stylistic choice a developer makes deliberately, and a surprise
  there reads as a bug.
- **Q13 — The system text factor on macOS.** Decision: 1.0 unless `/plan`
  finds a supported source (R68); Windows and Linux sources are researched in
  `/plan`, not assumed.
- **Q14 — New-project default: resolved at run time from the project, or
  written into each form?** Decision: written into each new form (R75), unlike
  the form theme (which resolves at run time — §9 F28). A form's responsive
  switch changes how its controls are laid out, so it must travel with the
  form.

## 8. What "spectacular" means here (the bar for `/plan`)

The operator's success criterion is that an experienced React developer would
be envious. Concretely, `/plan` must deliver, not merely permit:

1. **Live reflow in the designer as the canvas is dragged**, with no lag and
   no snapping — the layout function is pure and cheap enough to run every
   frame (it is arithmetic over a few dozen rectangles).
2. **The anchor gizmo on the selected control** — the visual language of
   Xcode's Auto Layout pins and Figma's constraints, on a COBOL form.
3. **A breakpoint bar above the canvas** — `Compact · Medium · Expanded`, one
   click to design each shape, overrides marked on the property rows, "reset
   to base" one click away.
4. **Flex and grid you can see** — gaps, item bounds and track lines drawn on
   the selected container, drag-to-reorder inside it.
5. **Dock that composes** — sidebar + header + fill, in three clicks, nested
   in a Panel that is itself docked, holding a grid of cards that reflows from
   five columns to one.
6. **Nothing breaks.** AC2's golden test over every form of PowerDemo3 and
   PowerChat is the promise to every existing user, and it is the first test
   written (§4.16).

## 9. Code read for this spec (evidence; `/plan` re-reads every one)

Re-read 2026-09-29 against branch `1.80.x` at `0241901`. Paths are under
`crates/` unless stated.

| # | Fact | Where |
|---|---|---|
| F1 | `Form` is a struct of typed fields (`name`, `title`, `width`, `height`, `transparency`, `grid_size`, `snap_to_grid`, `target`, …), not a property bag; `Form::new` sets the defaults | `cobolt-forms/src/model.rs:7419-7523`, `Form::new` `7526-7595` |
| F2 | `.cfrm` `<Form>` attributes: read in the `b"Form"` arm with `get_attr(…)` + defaults (a default-false bool is `main-form`, `:297-299`); a new attribute needs the enum variant (`:177-212`), the parse (`:254-340`), the construction (`:341-373`) and the assignment in `read_form` (`:483-544`); written in `form_to_string` (`:1424`, attributes `:1432-1527`), non-default booleans only when set (e.g. `main-form` `:1475-1478`); `get_attr_bool` (`:162`) is unused | `cobolt-forms/src/xml.rs` |
| F3 | Root form (window and shell) and pane occupants all render inside `ScrollArea::both()` with `ui.set_min_size(form_size)` and `RenderInput { form_size }` at the designed size; occupants take the pane below the breadcrumb band | `cobolt-form-host/src/host.rs` root `5143`, `5382-5412`; occupant band `5249-5257`; `child_frame` `2592`, `2718-2737` |
| F4 | The run window is created resizable at the designed inner size; no minimum inner size is set anywhere in `cobolt-form-host` or `cobolt-cli` | `cobolt-form-host/src/host.rs:292-316` (`with_inner_size` `304`, `with_resizable(true)` `305`) |
| F5 | `Anchor` is a **boolean drag-lock** (`is_anchored`); legacy string values (`"Top,Left"`) read as unanchored; real forms carry 663 `false`, 1 `true`, 2 `Top,Left` (`examples/PowerDemo3/forms/Data/datagrid-form.cfrm`, plus two `.bak` copies in `forms/Common/`) | `cobolt-forms/src/model.rs:6350-6360`; `cobolt-compiler/src/lib.rs:4948, 5160`; `cobolt-ide/src/panels/designer.rs:4702, 12905, 13055` |
| F6 | **Revision 1 said `AutoSize` was never read — now false.** A Label with `AutoSize` takes its caption's size (`label_autosize_rect`), PictureBox `SizeMode = AutoSize` the image's; the designer rewrites the stored rect, and at run time `render_form` resizes the live rect when COBOL changes the caption — a render-time rect rewrite this spec must order (R26) | seed `cobolt-forms/src/model.rs:4819`; `paint.rs:14228-14262`, `14264+`, `14289`; `render.rs:2310-2314`; `designer.rs:8342-8346`; `properties.rs:6112`; KB `cobolt-compiler/src/lib.rs:5202` |
| F7 | `Control::content_rect()` is the container client area (GroupBox/Panel inset 2, TabControl minus the strip per `TabPosition`); `containers::clip_rect` intersects them up the parent chain | `cobolt-forms/src/model.rs:6235-6275` (`tab_strip_extent` `6336`); `containers.rs:201-229` |
| F8 | Control rectangles are form-space absolute with `parent` links | `cobolt-forms/src/model.rs:14`; `containers.rs:10-11, 209-212`; `render.rs:1860` |
| F9 | The compiled binary's GUI path uses `cobolt_form_host` (same host as Run Form), window or shell | `cobolt-cli/src/form_gui.rs:35-40, 78-79, 673-707` |
| F10 | Render-time rect rewrites in `resolved_rect`: Splitter pane reflow (`splitter_child_rect` `1935`, `reflow_in_subtree` `1984`, `splitter_pane_rect` `1998`), **and descendants of a container COBOL moved follow it** (`1857-1881`, `moved_ancestor_offset` `1891`); SideMenu `rail_view` / `slide_content`; `expand_repeating_groups` | `cobolt-forms/src/render.rs:1624, 1823-1828, 1844`; `cobolt-forms/src/sidebar.rs:660, 701` |
| F11 | Owner-positioned controls are excluded from dragging: `is_anchored() \|\| is_side_menu_footer() \|\| is_splitter_pane()` | `cobolt-ide/src/panels/designer.rs:12905-12907`; `cobolt-forms/src/model.rs:6559, 6573` |
| F12 | `AnimationDef` carries `slide_dx`/`slide_dy` applied at paint time; designed rects untouched | `cobolt-forms/src/model.rs:1075-1091` |
| F13 | `TARGET_PRESETS` (phones, tablets, watches, desktop); `target_preset_size()`; the `"Target"` arm sets `form.width/height` | `cobolt-ide/src/panels/designer.rs:14117-14157, 15494-15502, 6397-6403` |
| F14 | `seed_theme_owned_appearance` is the one seeding function `Control::new` and `xml::seed_missing_props` both use; `border_style_default` the shared table; drift test over `ControlType::ALL` | `cobolt-forms/src/model.rs:4536-4640, 4649-4663, 6111, 9961`; `xml.rs:643-889` |
| F15 | Snackbar `StackAnchor` (nine positions, default `BottomRight`) is independent of `Anchor` by test | `cobolt-forms/src/model.rs:5902-5908`; `cobolt-forms/tests/snackbar_template.rs:247-290` |
| F16 | `RenderOutput.control_rects` records where the engine put every control; headless tests drive `render_form` and read it | `cobolt-forms/src/render.rs:357-364`; `cobolt-forms/tests/test_datagrid_filter_clip.rs:74` |
| F17 | Form-level inspector rows push through `action.form_props` and apply in `set_form_prop_direct` via the case-insensitive `canonical_form_prop_key`; labels are `Tr` fields defined six times | `cobolt-ide/src/panels/properties.rs:650, 10037-10767`; `cobolt-ide/src/app.rs:7090, 18605`; `designer.rs:6162, 6322-6403, 13854`; `cobolt-ide/src/i18n.rs:1536-1542` and the six tables |
| F18 | KB control-property docs: `UNIVERSAL_PROPS` + `property_reference_for`/`property_reference`; **no form-level property table** | `cobolt-compiler/src/lib.rs:4932-4960, 5007, 5092` |
| F19 | `Backdrop.window_size` carries the surface size into `render_form_inner` (backdrop = `max(form_size, window_size)`); the host fills it from `content_rect()` or the panel extent, Pane mode passes `None`; the preview sets it from `ui.available_size()` — the host knows the available size at the render call | `cobolt-forms/src/render.rs:209, 236-245, 415-417, 2147, 2169-2172`; `cobolt-form-host/src/host.rs:2332, 2380, 2700, 5288, 5360`; `cobolt-ide/src/app.rs:16340` |
| F20 | eframe 0.36.0 / egui 0.36.1; egui declared in five crates (`cobolt-cli`, `cobolt-forms` optional, `cobolt-form-host`, `cobolt-ide`, `cobolt-media`), eframe in three (`cobolt-cli`, `cobolt-form-host`, `cobolt-ide`) | `Cargo.lock`; `cobolt-cli/Cargo.toml:47-48`, `cobolt-forms/Cargo.toml:62`, `cobolt-form-host/Cargo.toml:41-42`, `cobolt-ide/Cargo.toml:46-47`, `cobolt-media/Cargo.toml:17` |
| F21 | **New since revision 1:** both host render paths call `Form::stretch_window_bars` every frame, widening each Responsive MenuBar and every StatusBar to the visible width; the preview does not | `cobolt-forms/src/model.rs:7769`; `cobolt-form-host/src/host.rs:2724, 5388` |
| F22 | COBOL geometry reads (`X`, `Y`, `Width`, `Height`) are served from the object store: seeded at start-up from each designed `c.rect` (form `Width`/`Height`/`X`/`Y` likewise), then changed only by the program's own writes; **the host never sends a control's live rectangle back**; `Left`/`Top` are not properties anywhere | `cobolt-runtime/src/interpreter.rs:14501-14507, 13895-13910, 14025`; `cobolt-runtime/src/objects.rs:54-61, 219-221`; `cobolt-form-host/src/seeding.rs:228, 254-257, 302-305` |
| F23 | The preview draws at the designed size in `ScrollArea::both`, `set_min_size(form_w, form_h)`, `RenderInput { form_size }`, via `render_form_with_chrome` | `cobolt-ide/src/app.rs:15510-15557, 15986, 16137-16142, 16320-16355`; `cobolt-forms/src/render.rs:2116` |
| F24 | **Since 1.70.341/343:** the host observes the form's size every frame — the window's inner size, or in a shell the ContentPane plus the rail's designed column (`rail_dx`) — mirrors `Width`/`Height` onto the form object, raises `onResizing` while it changes and `onResize` once it settles on a new size; a rail toggle resizes the shell window by the rail's width so the pane keeps its size (`shell_width_for_pane`) and is therefore not a resize of the form | `cobolt-form-host/src/host.rs:4918-5004` (fields `3090-3097`, init `591-593`, `side_dx` `429`); `cobolt-form-host/src/shell.rs` `shell_width_for_pane`, `resize_window_for_pane` |
| F25 | The Viewer lays out CSS flex and grid **by painting**: `paint_box_content` measures items with egui galleys and moves painted shapes to align them; its value types and track parser are egui-free; its shrink is a heuristic; `order`, reverse directions, `align-content` and `grid-row` are unsupported; `@media` compares against a fixed `MEDIA_WIDTH_PX = 1024`, once, at parse time | types `cobolt-forms/src/viewer.rs:719-773`, parse `3606-3695`, `css_grid_tracks` `831-880`; solver `paint.rs:10137-10368`, `10383-10480`, `max_content_width` `10487-10500`; `css.rs:62-64, 133-141, 589-625` |
| F26 | A `Padding` property appears on saved controls (e.g. a Shape in `examples/PowerChat/forms/chat-form.cfrm` before 0241901); whether it is seeded on containers and read by paint is for `/plan` to read | `git show 0241901^:examples/PowerChat/forms/chat-form.cfrm` (the removed `Shape-1`) |
| F27 | Font sizes: `ctrl_font_size` reads `FontSize` via `as_i64` (default 11 when missing, clamp 4–200); `as_i64` neither trims nor accepts decimals, so `"18.5"` paints at 4; several sites parse `FontSize` themselves with other defaults and bounds (render.rs `7461`, `8925`, `9090`, `11671`, `11687`; chart `paint.rs:12806, 12992` with `CHART_FONT_SCALE` `12495`; tab strip `model.rs:6282`); new controls are seeded 14; **no form-level font, zoom or scale exists**, the designer has no zoom, and **nothing reads an OS text-size setting** — only `native_pixels_per_point`, for `onDpiChanged`; a COBOL `FontSize` write does reach the paint | `cobolt-forms/src/paint.rs:16809-16813`; `model.rs:69-73, 4705-4710`; `fonts.rs:244-259`; `cobolt-form-host/src/host.rs:4739-4750`; `cobolt-runtime/src/interpreter.rs:14025-14038`; `cobolt-form-host/src/state.rs:152-156`; `render.rs:3360` |
| F28 | New projects: `create_new_project_at` → `CoboltProject::new` (`structure: CURRENT_STRUCTURE`, `built_with_version` empty, `forms: FormsConfig::new_project_defaults()`); **`new_project_defaults()` vs `impl Default for FormsConfig`** is the existing split between what new projects get and what old manifests fall back to (038's entrance effect); `built_with_version` is stamped only after a successful full build; the compiler keeps its own copy of the manifest types; the form theme is a project default **resolved at run time** (`resolve_theme_id`); new forms are made by `create_new_form` / `save_new_form_to` | `cobolt-ide/src/app.rs:4312-4422, 13424-13510, 14590-14595, 2291-2315`; `cobolt-ide/src/project_model.rs:39-77, 331-433, 438, 597-639, 793-827`; `cobolt-compiler/src/lib.rs:689, 789, 857`; `cobolt-forms/src/theme.rs:199-207` |
| F29 | Project upgrades are **offered, never imposed**, keyed on `[project] structure` (`CURRENT_STRUCTURE = STRUCTURE_MAIN_FORM_SEAL = 1`), one registered upgrade (`MainFormSeal`); a new project is born current | `cobolt-ide/src/project_upgrade.rs:7-35, 44-52, 55-104, 109-157, 273, 281`; `cobolt-ide/src/app.rs:12059-12140` |
