# Spec — Form container controls (real containment & reparenting)

- **Status:** draft → approved
- **Folder:** specs/012-form-container-controls/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-20

## 1. Overview

Turn `GroupBox`, `Panel`, and `TabControl` into **true containers**: controls can
be placed *inside* them as real parent/child relationships (not just visual
overlap), nested to any depth and in any combination. Children are positioned
relative to their container, move with it, and are clipped to the container's
visible rounded bounds — optionally scrolling when they overflow. The designer
supports reparenting by drag-and-drop (into the form, into another or the same
container, or adopting a neighbour's parent), and `TabControl` hosts a distinct
set of children per tab, showing only the active tab's controls. The data model
already carries a `children` tree and `.cfrm` serialises it, but the designer
never creates nesting, the runtime draws every control at **absolute**
coordinates with no clipping/tab-scoping, and codegen/runtime don't honour the
tree — this spec wires containment end-to-end (model ↔ designer ↔ renderer ↔
runtime ↔ codegen).

## 2. Goals / Non-goals

### Goals
- Real parent/child containment for `GroupBox`, `Panel`, `TabControl`, nestable to
  arbitrary depth and any combination (Panel ⊃ GroupBox ⊃ TabControl ⊃ Panel …).
- Per-tab child grouping for `TabControl` with show/hide on tab switch.
- Child coordinates **relative** to the container; container move/resize moves its
  children; clipping to the container's visible bounds respecting a configurable
  **border radius**.
- Per-container **auto-scroll** option: scroll overflowing content when on, clip
  when off.
- A **working `Opacity`** property for containers (fades the container and its
  children) — today the property exists but the renderer ignores it.
- Designer reparenting by drag-and-drop with the exact drop-target rules below,
  plus container-aware selection/move/delete and a drag-time drop affordance.
- Full round-trip in `.cfrm`; runtime + codegen honour the nested tree.

### Non-goals
- Anchoring / docking / auto-layout managers (children keep explicit positions).
- New container *types* beyond the three above.
- Multi-select drag *across* different parents in one gesture (single-parent drag
  is enough; multi-select within one parent is fine).
- Live reflow/resize of children when a container resizes (children keep their
  relative offsets; no proportional layout).

## 3. User stories
- As a form designer, I drag a TextBox onto a Panel and it becomes the Panel's
  child — moving the Panel moves the TextBox with it.
- As a form designer, I nest a GroupBox inside a Panel and a TabControl inside the
  GroupBox, and everything saves, reloads, and runs correctly.
- As a form designer, I put different controls on each tab of a TabControl and
  only the selected tab's controls show — at design time and at run time.
- As a form designer, I drag a control out of a container onto the canvas and it
  returns to the form; I drag it over another container and it moves into it.
- As a form designer, I give a Panel rounded corners and its children are clipped
  to that rounded shape; if I turn auto-scroll on, overflowing children scroll.

## 4. Requirements (EARS)

**Containment & nesting**
- **R1 (ubiquitous):** `GroupBox`, `Panel`, and `TabControl` shall be containers
  that own child controls; any container shall be able to contain any control,
  **including other containers, to arbitrary depth and in any combination**.
- **R2 (ubiquitous):** a child's position shall be **relative to its container's
  content origin**; moving (or resizing) a container shall move its children with
  it, preserving their relative offsets.
- **R3 (TabControl):** each tab page shall own a **distinct set** of child
  controls; only the **active tab's** children shall be visible and interactive;
  switching the selected tab shall show/hide the corresponding children — at
  design time and at run time.

**Clipping, radius, scroll**
- **R4 (ubiquitous):** child content shall be **clipped to the container's
  visible content area**, respecting the container's border radius.
- **R5 (ubiquitous):** containers shall expose a **configurable border-radius**
  property.
- **R6 (optional):** each container shall expose an **auto-scroll** option —
  where enabled and the children overflow the content area, the container shall
  scroll its content (scrollbars), at design time and run time; where disabled,
  the overflow shall be clipped (no scrolling).
- **R6b (ubiquitous):** the container's **`Opacity`** property (0–100) shall
  actually affect rendering — it shall fade the container's rendered output
  (background, border, and its child subtree) accordingly, at design time and run
  time. *(Today the property exists on controls but the renderer ignores it, so it
  has no visible effect — this requirement makes it work for containers.)*

**Designer reparenting (drag-and-drop)**
- **R7 (event):** when a control is dropped onto the **form canvas** (not over any
  container's content area), it shall be reparented to the **form**.
- **R8 (event):** when a control is dropped over a **container's visible content
  area** (another container, or the same one), its parent shall be updated to that
  container; dropping on the same container updates only its position.
- **R9 (constraint):** a container shall be a valid drop target **only over its
  visible content area**; hidden, clipped, or scrolled-out regions (and inactive
  tab pages) shall **not** accept drops.
- **R10 (event):** when a control is dropped over a **non-container control**, the
  dragged control shall receive the **same parent** as the control it was dropped
  over.
- **R11 (ubiquitous):** the designer shall render nested children and make them
  selectable, movable, and deletable; hit-testing/selection shall be
  container-aware (topmost child under the cursor wins) and scoped to the active
  tab for `TabControl`.
- **R12 (event):** while a drag would reparent, the designer shall visually
  indicate the target container / drop area.

**Lifecycle & ordering**
- **R13 (event):** deleting a container shall delete its children (cascade), as a
  single undoable action.
- **R14 (ubiquitous):** z-order shall be scoped within a parent — children stack
  among their siblings inside their container.
- **R15 (state):** while reparenting, the control's on-screen position shall be
  preserved by converting its coordinates between the old and new parent spaces.

**Persistence, runtime, codegen**
- **R16 (ubiquitous):** save/load (`.cfrm`) shall round-trip the full nesting —
  parent/child structure, per-tab assignment, and relative coordinates.
- **R17 (ubiquitous):** the runtime shall render the nested tree honouring
  relative coordinates, clipping (with radius), tab-scoping, and auto-scroll.
- **R18 (ubiquitous):** form→COBOL generation shall include **every** control
  regardless of nesting depth; the generated-code banner and the
  regenerate-on-Build/Run/Debug/Check contract are unchanged.

**Cross-cutting constraints**
- **R19 (constraint):** control ids shall remain **unique form-wide**, so
  `control::property` access, `INVOKE`, and event bindings are unaffected by
  nesting (a control is addressed by id, not by path).
- **R20 (constraint):** every new user-facing IDE string shall be a `Tr` field in
  **all six** languages (EN/ES/PT/JA/ZH/FR).
- **R21 (constraint):** the English `docs/developers-guide-en.md` shall document
  containers, nesting, the reparenting/drop rules, border-radius clipping, per-tab
  grouping, and auto-scroll; translations untouched.

## 5. Acceptance criteria
- [ ] **AC1 (R1,R2,R16,R17)** — A form with `Panel ⊃ GroupBox ⊃ TabControl ⊃
  Panel ⊃ TextBox` builds, saves, reloads identically, and renders with each child
  positioned relative to its parent; moving the outer Panel moves the whole tree.
- [ ] **AC2 (R3,R11,R17)** — A `TabControl` with controls on tab 1 and tab 2 shows
  only the active tab's controls and hides the rest, at design time and run time;
  switching tabs swaps which controls are visible/interactive.
- [ ] **AC3 (R4,R5,R17)** — Setting a container's border radius clips its children
  to the rounded bounds (a child extending past the rounded corner is cut).
- [ ] **AC4 (R7,R15)** — Dragging a contained control onto the bare form canvas
  reparents it to the form, keeping its on-screen position.
- [ ] **AC5 (R8,R9,R15)** — Dropping a control over another container's content
  area moves it into that container; over the same container only repositions it;
  a drop over a clipped/scrolled-out or inactive-tab region is rejected.
- [ ] **AC6 (R10)** — Dropping a control over a non-container control makes the
  dragged control a sibling (same parent) of that control.
- [ ] **AC7 (R6)** — With auto-scroll **on**, children overflowing a container
  scroll within it (scrollbars) at design and run time; with auto-scroll **off**,
  the overflow is clipped and does not scroll.
- [ ] **AC7b (R6b)** — Setting a container's `Opacity` below 100 visibly fades the
  container (background, border, and its children) at design time and run time;
  `Opacity = 100` is fully opaque (unchanged look), `0` is fully transparent.
- [ ] **AC8 (R13,R14)** — Deleting a container removes it and all descendants in
  one undoable step; z-order changes affect only siblings within a parent.
- [ ] **AC9 (R16,R18,R19)** — `.cfrm` round-trips the tree; the generated `.cbl`
  contains the nested children; `NestedTextBox::Text` (a control inside a
  container) is still addressable by id at runtime.
- [ ] **AC10 (R20,R21)** — New UI strings exist in all six languages
  (`cargo test -p cobolt-ide i18n` green); §-update lands in the English guide.

## 6. Constraints & steering check
- **i18n (6 languages):** new strings — drop-target hint, any reparent
  context-menu/labels, auto-scroll/border-radius property labels — must be `Tr`
  ×6 (EN/ES/PT/JA/ZH/FR). *(Note: the existing chart/property-pane labels use
  inline literals; new container strings follow the steering rule and use `Tr`.)*
- **Generated-code / regenerate contract:** codegen must emit nested children
  (today it likely walks only top-level controls — to confirm in /plan); the
  banner + regenerate-on-action behaviour is unchanged.
- **Docs:** English guide updated (R21); translations never touched.
- **Fix vs feature:** substantial new capability → **feature** (minor bump +
  CHANGELOG) per `tech.md`. *Open:* the operator's standing "treat session changes
  as fixes" directive may override the version classification — confirm at /plan or
  commit time.
- **No "cobolt" in user text; COBOL identifiers/source English.**

## 7. Open questions
- **Q1 (TabControl child model):** how is per-tab assignment represented in the
  model/`.cfrm` — a tab-page node owning children, or a `tab` index on each child
  of the `TabControl`? *Recommendation:* a tab-page grouping under the
  `TabControl` (clean show/hide + reorder). Resolve in /plan.
- **Q2 (border-radius defaults):** default radius per container — **0** (square,
  current look), configurable upward? *Recommendation:* default 0 so existing
  forms are visually unchanged.
- **Q3 (auto-scroll default & property name):** default **off** (clip), reusing
  `Panel`'s existing `Scrollable` semantics and extending it to `GroupBox` /
  `TabControl` pages. *Recommendation:* name it `AutoScroll`; keep default off.
- **Q4 (delete behaviour):** cascade-delete children (R13) vs. reparent them to
  the grandparent. *Recommendation:* cascade-delete (matches RAD tools), undoable.
- **Q5 (cycle/depth guard):** arbitrary depth is allowed (R1); reparenting must
  reject dropping a container **into its own descendant** (cycle). Confirm guard
  in /plan.
