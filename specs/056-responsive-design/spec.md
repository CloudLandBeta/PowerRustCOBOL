# Spec — Responsive design: edge anchoring and docking

- **Status:** draft → approved
- **Folder:** specs/056-responsive-design/
- **Author:** Claude (Fable 5.1) with the operator   **Date:** 2026-09-04

> **Standing rule for every phase of this feature — spec, plan, tasks,
> implement, docsync: DO NOT MAKE ASSUMPTIONS. READ THE CODE BEFORE
> IMPLEMENTING.** Every design statement in this spec cites the file and line
> it was read from (§9). `/plan` must re-read each cited site and cite every
> call site it touches; every task in `tasks.md` must name the code it read
> before changing it; `/implement` must verify each surface with a test that
> renders, never by inference from another surface. A claim about how the
> engine behaves that is not backed by a file:line or a test is not admissible
> in any artifact of this feature.

## 1. Overview

PowerRustCOBOL forms are laid out in absolute form-space pixels: every control
carries an `x, y, w, h` rectangle, the run-form window opens at the designed
size, and if the window is made larger the form stays its designed size in the
top-left corner while the rest is empty; made smaller, the form scrolls. Nothing
moves, stretches or reflows. This is the single largest gap between what the
platform *paints* (glass, gradients, rounded clipping, easing) and what a
current application is expected to *do* when its window, screen or pane
changes size.

This feature adds **responsive design** to a form as an opt-in, form-level
property — **"Responsive design"**, default **off**, so every existing form
keeps its exact current behaviour — under which each control declares how it
follows the edges of its parent (**anchoring**) or claims an edge of it
outright (**docking**), with minimum and maximum sizes. One pure, shared layout
function computes where every control lands for the surface it is being drawn
on, and every surface — the designer canvas, the preview, the run-form window,
a form loaded into a shell's ContentPane, and the compiled binary — draws from
that same answer, so the pixel-parity promise the platform already makes is
kept rather than bent.

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
  pane occupant and the compiled binary all obtain control rectangles from the
  same function with the same inputs, and a test proves they agree.
- G4 — The designer shows the layout **live**: drag the canvas edge and watch
  the form reflow; pick a device target and see the form at that size without
  changing what is designed; see a control's anchors on the control itself.
- G5 — Everything is deterministic and pure: the layout is a function of the
  designed form and the available size, idempotent, unit-testable without a
  window, and it never rewrites the design.
- G6 — Reclaim the property names a developer expects: `Anchor` means edges
  again (it once did — see §9 F5) and the canvas drag-lock that took the name
  gets its own.

**Non-goals** (explicitly out of scope for this spec)

- Flow, flex, grid or constraint-solver layouts. Anchoring and docking are the
  model; a flow container is a later spec that can build on this one.
- Per-breakpoint layouts ("this control is hidden below 600 px"). The device
  target preview in G4 is the seed for it, not the feature.
- Automatic font scaling with window size.
- Rewriting how `Splitter`, `SideMenu` or repeating groups position their own
  contents. They keep owning what they own (§4.7); this spec defines the order
  in which they run relative to the new layout, nothing more.
- Any change to generated COBOL. Layout is host-side.
- Reusing the dead provisions found in the code: the legacy string `Anchor`
  values, the never-read `AutoSize` property (§9 F6). They are documented so
  they are not reused, not so they are revived.

## 3. User stories

- As a **form developer**, I want a form to stretch to the window the user
  gives it, so that a grid fills a large screen instead of leaving a third of
  it empty and a status bar stays at the bottom instead of floating mid-window.
- As a **form developer**, I want to say "this panel is the left sidebar, this
  toolbar sits along the top, and the grid takes whatever is left", so that I
  never compute a size by hand again.
- As a **form developer**, I want to design for one device size and watch the
  form at an iPhone, an iPad and a desktop width in the designer, so that I
  find layout problems before running anything.
- As a **maintainer of existing forms**, I want every form I already have to
  render exactly as it does today until I choose otherwise, so that adopting
  this feature is a decision per form, never an event that happens to me.
- As a **COBOL programmer**, I want a control's `::Width` and `::Height` to
  tell me the size it actually has on screen, so that my code and the screen
  never disagree.
- As a **reviewer of this feature**, I want every claim about the engine in the
  plan and tasks to point at the line it was read from, so that the feature is
  built on the code that exists rather than on the code someone remembers.

## 4. Requirements (EARS)

### 4.1 The form-level switch

- **R1 (ubiquitous):** The system shall give every form a boolean property
  **`Responsive design`** (model field `responsive`, `.cfrm` attribute
  `responsive`), following the form-attribute conventions already in use
  (§9 F2).
- **R2 (ubiquitous):** The system shall default `Responsive design` to
  **false**, and shall treat a `.cfrm` that carries no `responsive` attribute
  as false, so that no existing form changes behaviour on load.
- **R3 (state):** While `Responsive design` is false, the system shall render
  the form on every surface **exactly as it does before this feature** — the
  designed size inside a scrolling surface (§9 F3) — and a test shall prove
  the control rectangles are identical.
- **R4 (event):** When the developer toggles `Responsive design` in the
  designer, the system shall apply it live to the canvas and mark the form
  dirty, and shall not move, resize or rewrite any designed rectangle.
- **R5 (constraint):** The system shall not require any other change to a form
  for `Responsive design` to be turned on: with the switch on and every
  control at the default anchor (§4.2), a form shall render identically to the
  switch off at the designed size, and simply stop scrolling beyond it.

### 4.2 Anchoring

- **R6 (ubiquitous):** The system shall give every visual control an **`Anchor`**
  property whose value is a set of edges drawn from `Top`, `Bottom`, `Left`,
  `Right`, persisted as a comma-separated string (e.g. `"Top,Left,Right"`).
- **R7 (ubiquitous):** The default `Anchor` shall be `"Top,Left"`, which is by
  definition today's behaviour: a fixed offset from the top-left of the parent.
- **R8 (state):** While a form is responsive, the system shall, for each axis
  independently, position a control according to which of that axis's two
  edges are in its `Anchor`:
  - **leading only** (`Left` / `Top`): keep the designed offset from the
    leading edge; size unchanged;
  - **trailing only** (`Right` / `Bottom`): keep the designed offset from the
    trailing edge; size unchanged;
  - **both**: keep both offsets, so the control's size on that axis changes by
    the parent's change on that axis (stretch);
  - **neither**: keep the control's **centre** at the same fraction of the
    parent's extent (proportional), size unchanged.
- **R9 (ubiquitous):** All anchor arithmetic shall be computed against the
  **designed** rectangle and the **designed** parent size (the `.cfrm` values),
  never against the previous frame's result, so that the layout is idempotent
  and cannot drift under repeated resizes.
- **R10 (constraint):** The system shall never write a laid-out rectangle back
  into the design. The designed rectangles remain the source of truth; layout
  is a view of them.

### 4.3 Docking

- **R11 (ubiquitous):** The system shall give every visual control a **`Dock`**
  property with the values `None`, `Left`, `Top`, `Right`, `Bottom`, `Fill`,
  default `None`.
- **R12 (state):** While a form is responsive, the system shall lay out docked
  controls in **render (z-) order**, each consuming one edge of its parent's
  remaining client rectangle: `Left`/`Right` take the full remaining height at
  the control's designed width; `Top`/`Bottom` take the full remaining width at
  the control's designed height; `Fill` takes the whole remaining rectangle.
  After each docked control is placed, the remaining rectangle shrinks by what
  it took.
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
- **R17 (state):** While laying out, the system shall clamp every stretched or
  docked dimension to `[Min, Max]` (a non-zero bound applies; a zero bound does
  not), and shall place the clamped control according to its anchors from the
  edges it is anchored to — a control anchored `Left,Right` that hits its
  `MaxWidth` stays attached to `Left`.
- **R18 (ubiquitous):** The system shall compute the form's **minimum size** —
  the smallest surface at which every docked control receives at least its
  minimum and no anchored control is clamped below its minimum — and, on a
  resizable run-form window, shall set the window's minimum inner size to it,
  never below 64 × 64, so the OS resize handle cannot produce a layout the
  form cannot honour.

### 4.5 Containers

- **R19 (ubiquitous):** For a control whose `parent` is a container
  (`GroupBox`, `Panel`, `TabControl`), the parent rectangle used by §4.2 and
  §4.3 shall be that container's **client rectangle** as the engine already
  defines it — `Control::content_rect()` (§9 F7), which accounts for the tab
  strip on every `TabPosition` — computed from the container's **laid-out**
  rectangle.
- **R20 (ubiquitous):** Layout shall be recursive and top-down: a container is
  placed by its own parent first, then its children are placed inside it. A
  container's children shall never influence the container's own placement
  except through the form-minimum computation (R18).
- **R21 (ubiquitous):** Control rectangles shall remain **form-space absolute**
  in the model and in every output (§9 F8); layout produces absolute
  rectangles, not parent-relative ones, so that every existing consumer of a
  control's rectangle keeps working unchanged.

### 4.6 One layout, every surface

- **R22 (ubiquitous):** The system shall implement layout as **one pure
  function** in `cobolt-forms` — inputs: the designed controls, the designed
  form size, the available surface size; output: a rectangle per control —
  with no dependency on egui, on a window, or on any frame state, so that it
  is unit-testable and the same on every surface.
- **R23 (ubiquitous):** Every surface that draws a form into a rectangle shall
  obtain control rectangles from that function with the surface's own
  available size: the **run-form window** (its inner size), a **form loaded
  into a shell ContentPane** (the pane rectangle below the breadcrumb band —
  §9 F3), the **preview**, the **designer canvas**, and the **compiled
  binary**, which shares the run-form host (§9 F9). The plan shall list each
  call site by reading it; the tasks shall test each one.
- **R24 (ubiquitous):** The function shall be **idempotent**: applying it to
  its own output at the same available size yields the same output; and
  **pure with respect to size**: the same inputs always yield the same
  rectangles regardless of what was rendered before.
- **R25 (event):** When a responsive form's available size changes at run
  time, the system shall recompute the layout **in the same frame** the new
  size is observed, with no intermediate frame drawn at the old layout.

### 4.7 Order of precedence with what already moves controls

The engine already has three mechanisms that rewrite control rectangles at
render time (§9 F10). This spec does not change them; it fixes their order.

- **R26 (ubiquitous):** The responsive layout shall run **first**, on the
  designed rectangles, producing laid-out rectangles; the existing mechanisms —
  Splitter pane reflow, SideMenu rail narrowing and content slide, repeating-
  group instancing — shall then operate on **those** rectangles exactly as
  they operate on designed rectangles today.
- **R27 (ubiquitous):** Controls whose position is **owned by another
  mechanism** — a Splitter pane, a SideMenu footer, a repeating-group instance
  (§9 F11) — shall not be anchored or docked by the developer: their `Anchor`
  and `Dock` shall be ignored by layout and their rows hidden in the inspector,
  with the owner control itself (the Splitter, the SideMenu, the template
  group) remaining anchorable and dockable as a whole.
- **R28 (ubiquitous):** Animations that offset a control (`slide_dx`,
  `slide_dy`, §9 F12) shall apply on top of the laid-out rectangle, as they
  apply on top of the designed one today.

### 4.8 The designer

- **R29 (state):** While a form is responsive, the designer canvas shall show
  the form **laid out at the current canvas size**, and dragging the canvas
  resize grip shall reflow the controls live, while the designed rectangles —
  what is saved and what the property rows show — stay exactly what the
  developer set.
- **R30 (ubiquitous):** The designer shall offer a **"View at"** device-size
  selector drawing on the existing target presets (§9 F13), which sets the
  canvas's available size for viewing only and never changes the form's
  designed `Width`/`Height` or its `Target`.
- **R31 (ubiquitous):** When a control is selected on a responsive form, the
  designer shall draw an **anchor gizmo** on the control — one pin per edge,
  lit when that edge is in the control's `Anchor` — and clicking a pin shall
  toggle that edge. A docked control shall show its dock edge instead of pins.
- **R32 (ubiquitous):** The properties pane shall show `Anchor` (as four
  checkboxes), `Dock` (a choice), and the four size limits in a **Layout**
  section, for every control that is not owner-positioned (R27), and shall
  show the section **only when the form is responsive**, with a one-line hint
  where the section would be otherwise saying that `Responsive design` is off.
- **R33 (ubiquitous):** Moving or resizing a control on a responsive canvas
  shall edit its **designed** rectangle by the inverse of the current layout
  mapping, so that what the developer drags is what they see — never the raw
  designed value silently offset from the cursor.

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

- **R37 (ubiquitous):** A runtime read of a control's geometry properties from
  COBOL (`Left`, `Top`, `Width`, `Height` through the `::` surface) on a
  responsive form shall return the **laid-out** values for the surface the
  form is currently on.
- **R38 (event):** When COBOL writes a geometry property on a responsive form,
  the system shall treat the write as a change to the **designed** rectangle
  and re-run the layout, so that the write composes with anchoring rather than
  being overwritten by the next resize.
- **R39 (event):** When a responsive form's available size changes at run
  time, the system shall fire a form-level **`onResize`** event after the
  layout has been applied, so a handler observing geometry sees the new
  values.

### 4.11 Process constraints (carried into every phase)

- **R40 (constraint):** No artifact of this feature — plan, task, code,
  commit message, doc — shall assert how existing code behaves without citing
  the file and line that was read, or a test that demonstrates it.
- **R41 (constraint):** `/plan` shall re-read every site cited in §9 and shall
  enumerate, by reading, every place a control's rectangle is consumed on each
  surface, before choosing where the layout function is called.
- **R42 (constraint):** `/implement` shall verify each surface (R23) with a
  test that renders headlessly and reads `RenderOutput.control_rects`
  (§9 F16), never by reasoning that "the surfaces share code".

## 5. Acceptance criteria

Each criterion is a test unless marked *(manual)*; the tasks phase turns them
into named tests.

- [ ] **AC1 (R1, R2)** — A `.cfrm` without `responsive` loads with
  `responsive == false`; one with `responsive="true"` loads true; saving writes
  the attribute only when true (so untouched forms stay byte-identical on
  save).
- [ ] **AC2 (R3, G2)** — For every fixture form in the test corpus, rendering
  headlessly at three surface sizes (smaller, equal, larger than designed) with
  `responsive == false` yields `control_rects` identical to the pre-feature
  engine (a golden captured before the change). Zero drift permitted.
- [ ] **AC3 (R5)** — A responsive form whose controls all carry the default
  `Anchor` renders identically to the non-responsive form at the designed size.
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
- [ ] **AC7 (R16, R17)** — Stretch and dock respect Min/Max: a `Left,Right`
  control with `MaxWidth` stays attached to `Left` at max; a `Fill` control with
  `MinHeight` is never shorter than it.
- [ ] **AC8 (R18)** — The computed form minimum equals the hand-derived value
  for a fixture with docked and anchored controls, and the run-form window
  builder receives it as its minimum inner size.
- [ ] **AC9 (R19, R20, R21)** — Children of a `TabControl` on each of the four
  `TabPosition`s, of a `GroupBox`, and of a `Panel` lay out inside the
  container's `content_rect()` computed from the container's laid-out rect;
  every output rect is form-space absolute.
- [ ] **AC10 (R23, R42)** — One test per surface — run-form window, pane
  occupant, preview, designer canvas — renders the same responsive fixture at
  the same available size and asserts identical `control_rects` across all
  four. The compiled-binary path is covered by the run-form host test (§9 F9)
  plus a build-and-run smoke test.
- [ ] **AC11 (R26)** — A responsive form containing a Splitter, a collapsed
  SideMenu and a repeating group renders with each mechanism operating on the
  laid-out rects: the Splitter's pane children reflow from their laid-out
  positions; the rail's content slides from its laid-out position; instances
  expand from the laid-out template.
- [ ] **AC12 (R27)** — A Splitter pane, a SideMenu footer and a repeating
  instance with `Anchor`/`Dock` set are laid out by their owner, unchanged, and
  their inspector rows are hidden.
- [ ] **AC13 (R29, R33)** — Designer test: on a responsive canvas at a larger
  size, dragging a `Right`-anchored control by +10 px changes its **designed**
  `x` by exactly +10 px.
- [ ] **AC14 (R30)** — Selecting a "View at" preset changes the canvas
  available size and leaves `form.width`, `form.height` and `form.target`
  untouched.
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
- [ ] **AC18 (R37, R38, R39)** — A runtime program on a responsive form reads
  `::Width` after a resize and gets the laid-out width; writes `::Width` and the
  next layout composes with it; `onResize` fires once per size change, after
  layout.
- [ ] **AC19 (i18n)** — Every new label (`Responsive design`, `Layout`, `Anchor`,
  `Dock`, `Locked`, `View at`, the four limits, the "off" hint) exists in all
  six languages and the i18n completeness test is green.
- [ ] **AC20 (KB)** — The KB property tables carry `Anchor` (new meaning),
  `Dock`, `Locked`, `MinWidth`, `MinHeight`, `MaxWidth`, `MaxHeight`; the
  chunked store is rebuilt; `prebuilt_chunked_kb_matches_the_published_documentation`
  is green.
- [ ] **AC21 (docs)** — `docs/developers-guide-en.md` gains a section
  "Responsive design" and the support matrix gains rows for anchoring, docking
  and size limits under `PRC`; screenshots slots are left for `/doc-shots`.
- [ ] **AC22 (R40, R41)** — `plan.md` cites a file:line for every existing-code
  claim, and lists every rectangle consumer per surface; `tasks.md` names, for
  each task, the code read before the change. *(Checked in `/analyze`.)*

## 6. Constraints & steering check

- **i18n (6 languages):** new `Tr` fields for every label in AC19 —
  EN/ES/PT/JA/ZH/FR, in `crates/cobolt-ide/src/i18n.rs` (§9 F17). No literals.
- **Generated COBOL / regenerate contract:** unaffected. Layout is host-side;
  no codegen change; the regenerate-on-Build/Run/Debug/Check contract stands.
- **System KB:** required in the same change (tech.md hard constraint) —
  property tables in `cobolt-compiler`'s doc tables (§9 F18), plus the chunked
  store rebuild (`cargo run -p cobolt-ide --example build_chunked_kb`) and its
  committed `assets/knowledge/chunked.data`.
- **Docs:** English guide section + support-matrix rows (AC21). Translations
  are regenerated by the localization cycle on the next minor, not patched.
- **Fix vs feature:** **feature** — new user-visible functionality. Work
  happens on the `features` branch; per the operator's standing rule the agent
  bumps only `z` in `version.rs`; the forum announcement waits for the release
  candidate batch.
- **Verify-first:** every test reports what it measured; no acceptance
  criterion is ticked from a filtered grep (see the test-sweep rules in the
  project memory).
- **Pixel parity (product promise):** R23/AC10 are the guard.
- **Rust only:** the layout function and all tests are Rust; no scripts in the
  tree.

## 7. Open questions

Resolved by the operator's delegation ("I will rely on your decisions as long
as they are the best in the long run"); listed so the choice is visible and
reversible at approval:

- **Q1 — Reclaim `Anchor` for edges and move the drag-lock to `Locked`
  (R34/R35), or introduce a second name (`Anchors`) and leave the lock alone?**
  Decision: reclaim. Two near-identical names forever is the worse long-run
  outcome; the property was edges before it was a lock (§9 F5), WinForms and
  every developer's expectation agree, and the migration is one-time, at load,
  through the shared seeding path, pinned by a load test.
- **Q2 — Proportional anchoring (neither edge) — include or drop?** Decision:
  include (R8). It is the only way to keep a centred control centred, it costs
  one branch in a pure function, and it is what a React developer expects from
  a percentage position.
- **Q3 — Docked controls: designed width/height as the docked thickness, or a
  separate `DockSize`?** Decision: designed size. The canvas shows the
  thickness the developer drew; a second property would be a second truth.
- **Q4 — Should a non-responsive form gain the Layout inspector section
  greyed out?** Decision: hidden with a one-line hint (R32), so the pane does
  not grow by six rows on every form that has not opted in.
- **Q5 — `onResize`: form-level only, or per control?** Decision: form-level
  in this spec (R39); a per-control event is cheap to add later and has no
  compelling story yet.
- **Q6 — The window minimum (R18): derived from the layout, or the designed
  size?** Decision: derived. Setting the minimum to the designed size would
  make "resizable" mean "growable", which defeats the feature on small
  screens.

## 8. What "spectacular" means here (the bar for `/plan`)

The operator's success criterion is that an experienced React developer would
be envious. Concretely, `/plan` must deliver, not merely permit:

1. **Live reflow in the designer as the canvas is dragged**, with no lag and
   no snapping — the layout function is pure and cheap enough to run every
   frame (it is arithmetic over a few dozen rectangles).
2. **The anchor gizmo on the selected control** — the visual language of
   Xcode's Auto Layout pins and Figma's constraints, on a COBOL form.
3. **"View at" device presets** in the designer toolbar, one click from
   phone to desktop, never touching the design.
4. **Dock that composes** — sidebar + header + fill, in three clicks, nested
   in a Panel that is itself docked.
5. **Nothing breaks.** AC2's golden test is the promise to every existing
   user, and it is the first test written.

## 9. Code read for this spec (evidence; `/plan` re-reads every one)

| # | Fact | Where |
|---|---|---|
| F1 | `Form` is a struct of typed fields (`name`, `title`, `width`, `height`, `transparency`, `grid_size`, `snap_to_grid`, `target`, …), not a property bag | `crates/cobolt-forms/src/model.rs:6658-6684` |
| F2 | `.cfrm` `<Form>` attributes are kebab-case, read with `get_attr(e, b"snap-to-grid")`-style calls and defaults, written with `elem.push_attribute(("grid-size", …))`; `get_attr_bool` exists | `crates/cobolt-forms/src/xml.rs:254-300, 1282-1300, 162` |
| F3 | Run-form root path and pane-occupant path both render inside `ScrollArea::both()` with `ui.set_min_size(form_size)` and pass `form_size` in `RenderInput` — i.e. the designed size, scrolled; the occupant's rect is the pane below the breadcrumb band | `crates/cobolt-form-host/src/host.rs:1751-1790`, `child_frame` (+48…+110), `3365-3470` |
| F4 | The run window is created resizable at the designed inner size; no minimum inner size is set today | `crates/cobolt-form-host/src/host.rs:265-275` |
| F5 | `Anchor` is today a **boolean drag-lock** (`is_anchored`), documented as "Locks the control against mouse dragging"; the same comment records that string values like `"Top,Left"` were once anchor edges and are now treated as unanchored; real forms carry 623 `false`, 1 `true`, and 6 legacy `Top,Left` (in `Common/datagrid-form.cfrm` and its backups) | `crates/cobolt-forms/src/model.rs:5722-5733`, `crates/cobolt-compiler/src/lib.rs:3742,3820`, `crates/cobolt-ide/src/panels/designer.rs:11537,11687`, `~/Documents/PowerDemo3/forms` |
| F6 | `AutoSize` is seeded, documented ("Grows the control to fit its text") and shown in the inspector, but **never read** by `paint.rs`, `render.rs` or the designer — a dead property, not to be reused | `crates/cobolt-forms/src/model.rs:4478`, `crates/cobolt-compiler/src/lib.rs:3860`, `crates/cobolt-ide/src/panels/properties.rs:5693` |
| F7 | `Control::content_rect()` is the container client area: GroupBox/Panel inset by 2, TabControl minus the tab strip per `TabPosition`; `containers::clip_rect` intersects them up the parent chain | `crates/cobolt-forms/src/model.rs:5646-5680`, `crates/cobolt-forms/src/containers.rs:148-162` |
| F8 | Control rectangles are **form-space absolute** with `parent` links; nested children are not parent-relative | `crates/cobolt-forms/src/model.rs:14`, `containers.rs:145,193` |
| F9 | The compiled binary's GUI path uses `cobolt_form_host` (same host as Run Form) | `crates/cobolt-cli/src/form_gui.rs:39-78` |
| F10 | Three mechanisms already rewrite rectangles at render time: Splitter pane reflow (`resolved_rect` → `splitter_pane_rect` / `splitter_child_rect`, calling `splitter::reflow_in_subtree`), SideMenu `rail_view` (narrowing + `slide_content`), and `expand_repeating_groups` | `crates/cobolt-forms/src/render.rs:1279, 1499-1600`, `crates/cobolt-forms/src/sidebar.rs:632-690` |
| F11 | Owner-positioned controls are already excluded from dragging: `is_anchored() \|\| is_side_menu_footer() \|\| is_splitter_pane()` | `crates/cobolt-ide/src/panels/designer.rs:11537-11541`, `model.rs:5890,5904` |
| F12 | `AnimationDef` carries `slide_dx`/`slide_dy` offsets applied at paint time; animations do not mutate designed rects | `crates/cobolt-forms/src/model.rs:1016-1025` |
| F13 | `TARGET_PRESETS` lists phones, tablets, watches and desktop sizes; `target_preset_size()` sets `form.width/height` when the `Target` property changes | `crates/cobolt-ide/src/panels/designer.rs:12705-12740, 14013-14021, 5350-5356` |
| F14 | `seed_theme_owned_appearance` is the one seeding function both `Control::new` and `xml::seed_missing_props` call; `border_style_default` is the per-type table both read; a drift test walks `ControlType::ALL` — the pattern R35 must follow | `crates/cobolt-forms/src/model.rs:4206-4330`, `xml.rs:588-740`, `model.rs` tests `every_control_types_seeded_border_style_matches` |
| F15 | Snackbar has its own `StackAnchor` (nine positions, default `BottomRight`), independent of `Anchor` by test | `crates/cobolt-forms/src/model.rs:5422`, `tests/snackbar_template.rs:261-290` |
| F16 | `RenderOutput.control_rects` records where the engine put every control; headless tests already drive `render_form` with a `RenderInput` and read it | `crates/cobolt-forms/src/render.rs:349-356`, `crates/cobolt-forms/tests/test_datagrid_filter_clip.rs` |
| F17 | Form-level inspector rows push `(name, value)` through `action.form_props` and the designer applies them in a `match` (`"Width"`, `"Height"`, `"GridSize"`, `"SnapToGrid"`, `"Target"`); labels are `Tr` fields defined six times | `crates/cobolt-ide/src/panels/properties.rs:9500-9520`, `designer.rs:5280-5360`, `i18n.rs:1302,2610,3848,5086,6323,7567,8806` |
| F18 | KB control-property docs: `UNIVERSAL_PROPS` name list plus a `(domain, doc)` table per property; form-level properties have no table today | `crates/cobolt-compiler/src/lib.rs:3725-3860` |
| F19 | `Backdrop` already carries the surface's `window_size` into `render_form_inner`, which sizes the backdrop to `max(form_size, window_size)` — proof the host knows the available size at the render call | `crates/cobolt-forms/src/render.rs:188-210, 384-386, 1764-1772` |
| F20 | eframe is 0.36.0 / egui 0.36.1, declared per crate in five crates (`cobolt-ide`, `cobolt-cli`, `cobolt-form-host`, `cobolt-forms`, `cobolt-media`) | `Cargo.lock`, the five `Cargo.toml`s |

Not read for this spec and therefore **not asserted** (plan must read them):
the preview viewport's exact render call (`app.rs` `show_preview_window`, from
line 13788 — its animation setup was read, its paint call was not); how the
`::Left/Top/Width/Height` runtime reads are served today (R37); where the
form-host observes a window-size change per frame (R25).
