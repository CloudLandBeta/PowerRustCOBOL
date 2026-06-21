# Spec — Visual Repeating Groups (GroupBox arrays)

- **Status:** draft → approved
- **Folder:** specs/015-visual-repeating-groups/
- **Author:** Anthropic Claude Codex Agent (with Eslopes)   **Date:** 2026-06-21
- **Supersedes:** specs/014-copy-as-cobol-code/ (abandoned approach)

## 1. Overview

Let a developer **design a reusable group of controls once** in the Form Designer
and mark it as a **repeating array**. A `GroupBox` becomes the visual template for
a single repeated item; every control inside it is part of that template. At
runtime PowerRustCOBOL renders **N visual instances** of the group from one
design-time template — no manual control-creation code. Inner controls are
addressed as **indexed children** of each instance (`CustomerCard(3)::DetailsButton`),
events from any instance route to the **one shared handler** for that template
control, and the handler receives the **item index** so the developer knows which
instance fired. This builds directly on **spec 012** (GroupBox is already a true
container with relative child coords, border-radius clipping, auto-scroll, and
opacity) and **spec 011** (indexed `::` member-access chains over a nested object
model). The feature also rounds out the `GroupBox`'s visual properties
(hide caption/background, corner radius, background colour + gradient).

The work is staged in five phases (see §8). **Phases 1–2 are specified for the
first merge**; Phases 3–5 (runtime cloning, indexed event dispatch, data binding)
are specified at the requirements level and detailed in their own plan/tasks.

## 2. Goals / Non-goals

### Goals
- Extend `GroupBox` appearance: **HideCaption**, **HideBackground**,
  **corner radius**, **BackgroundColor**, and an optional **background gradient**
  (vertical / horizontal / diagonal / radial), with children clipped to the
  rounded bounds (reusing spec-012 clipping).
- A context-menu toggle to **mark a GroupBox as a repeating group** (and unset
  it), plus a **Repeating Group** property section.
- A **template model**: the `.cfrm` stores the group's children once plus
  repeating-group metadata; the runtime instantiates N visual copies.
- **Layout** of instances: Vertical, Horizontal, or Grid (with items-per-row and
  spacing); placeable inside an **auto-scroll Panel** (spec-012 `AutoScroll`).
- **Indexed addressing** of instances and their children via the existing `::`
  chain grammar: `ArrayName(i)::ChildId::Property`.
- **Shared event handlers** with a passed **array index** — one COBOL handler per
  template control, receiving the firing instance's index.
- **Design-time preview** of multiple instances without polluting the form model
  with duplicate design-time controls.
- Optional **data binding** so instances fill themselves from a data source.

### Non-goals
- New container *types* beyond `GroupBox` as the array template (Panel/TabControl
  remain plain containers from spec 012).
- Nested repeating groups (a repeating group **inside** another repeating group)
  — out of scope for v1; single-level arrays only. *(Flag, §7 Q6.)*
- Anchoring / docking / proportional resize of instances (instances keep the
  template's relative child offsets, per spec 012 non-goals).
- A full reactive data-binding/ORM layer — binding is incremental (Phase 5) and
  field-level, not a general expression engine.
- Re-implementing container basics already delivered by spec 012.

## 3. User stories
- As a developer, I design a `CustomerCard` GroupBox (name, balance, photo, trend
  chart, details button) once, mark it as a repeating group, and the runtime shows
  one card per data row.
- As a developer, I set the layout to Grid with 3 per row and the cards flow into
  rows automatically inside a scrolling panel.
- As a developer, I write **one** `onClick` handler for the card's Details button
  and, when any card's button is clicked, my handler runs with the index of the
  card that was clicked.
- As a developer, I set `CustomerCard(5)::CustomerName::Caption` from COBOL to
  update just the fifth card.
- As a developer, I preview 3 instances at design time to see the layout, while my
  form model still contains only the single template.
- As a developer, I bind `CustomerName::Caption` to a `Name` field and the cards
  populate from the data source with no per-field assignment code.

## 4. Requirements (EARS)

### A. GroupBox visual properties (Phase 1)
- **R1 (ubiquitous):** `GroupBox` shall expose **HideCaption** (Bool, default
  false) — hides the caption/title text while remaining a container.
- **R2 (ubiquitous):** `GroupBox` shall expose **HideBackground** (Bool, default
  false) — makes the box background transparent while keeping children visible
  (no card/frame fill drawn), analogous to the chart `HideBackground` fix.
- **R3 (ubiquitous):** `GroupBox` shall expose a **corner-radius** property and
  clip child content to the rounded visible bounds. *(Reuse the spec-012
  `BorderRadius` property rather than add a second `CornerRadius`; §7 Q1.)*
- **R4 (ubiquitous):** `GroupBox` shall expose **BackgroundColor** (Color,
  default theme-defined).
- **R5 (optional):** `GroupBox` shall expose an optional **background gradient** —
  `BackgroundGradientEnabled` (Bool), `BackgroundGradientStartColor`,
  `BackgroundGradientEndColor`, and `BackgroundGradientDirection`
  (Vertical | Horizontal | DiagonalDown | DiagonalUp | Radial). Where enabled, the
  box background shall render the gradient (reusing spec-013 gradient meshes);
  where disabled, it shall render the solid `BackgroundColor`.
- **R6 (constraint):** existing non-repeating `GroupBox` behaviour and appearance
  shall be unchanged when the new properties are at their defaults.

### B. Repeating-group configuration (Phase 2)
- **R7 (event):** when the developer right-clicks a `GroupBox`, the context menu
  shall offer **"Set as Repeating Group"**; when the GroupBox is already a
  repeating group, it shall instead offer **"Unset Repeating Group"**.
- **R8 (state):** while a `GroupBox` is a repeating group, the properties pane
  shall show a **Repeating Group** section exposing: **IsRepeatingGroup** (Bool,
  default false), **ArrayName** (String, default = GroupBox name), **ItemCount**
  (Int, default 0; runtime instance count), **DataSource** (String, default
  empty), **LayoutDirection** (Vertical | Horizontal | Grid, default Vertical),
  **ItemSpacing** (Int, default 8), **ItemsPerRow** (Int, default 1; used when
  Grid), **AutoScrollParent** (Bool, default true), **CloneEvents** (Bool,
  default true), and **PreviewItemCount** (Int, default 1).
- **R9 (ubiquitous):** repeating-group metadata shall be stored in the `.cfrm`
  as part of the `GroupBox`, round-tripping on save/load, and the template's
  children shall be stored **once** (not duplicated per instance).
- **R10 (ubiquitous):** the designer shall **visually indicate** a repeating
  group (e.g. an array badge / "Repeating Group" marker) while still allowing the
  developer to edit the template controls normally.
- **R11 (optional):** where `PreviewItemCount > 1`, the designer shall preview
  that many instances laid out per `LayoutDirection`, and the preview clones shall
  **not** be added to the form model (they are render-only).

### C. Runtime instancing & layout (Phase 3)
- **R12 (ubiquitous):** at runtime a repeating `GroupBox` shall act as a template;
  the runtime shall create one visual instance per array element
  (count from `ItemCount` / `DataSource`).
- **R13 (ubiquitous):** each instance shall preserve the template's relative child
  positions, child sizes, styles/properties, containment hierarchy, z-order, and
  (if a `TabControl` is inside the group) tab-page ownership.
- **R14 (ubiquitous):** instances shall be arranged per **LayoutDirection** —
  Vertical (stacked), Horizontal (side-by-side), or Grid (`ItemsPerRow` columns,
  wrapping into rows) — separated by **ItemSpacing**.
- **R15 (optional):** where `AutoScrollParent` is enabled and the instances exceed
  the parent's visible area, the parent container shall scroll (reusing spec-012
  `Panel` `AutoScroll`).

### D. Indexed addressing & events (Phases 3–4)
- **R16 (ubiquitous):** instances and their children shall be addressable by index
  via the existing `::` chain grammar (spec 011):
  `ArrayName(i)::ChildId::Property` shall read/write the property of `ChildId` in
  the i-th instance. *(Final exact form per spec-011 compatibility; §7 Q2.)*
- **R17 (constraint):** because a template control yields N runtime controls, the
  form-wide-unique-id rule (spec 012 R19) shall be satisfied by the
  **`ArrayName(i)::ChildId`** path; the bare child id alone shall not be required
  to be unique across instances.
- **R18 (state):** while `CloneEvents` is enabled, all instances of a given
  template control shall **share one** COBOL event handler (one handler per
  template control, not per instance).
- **R19 (event):** when an event fires from a cloned control, the runtime
  dispatcher shall resolve it to the **template control's** handler and shall make
  the firing instance's **array index** available to that handler.
- **R20 (ubiquitous):** the generated COBOL handler for a repeating-group control
  shall receive the array index through the `LINKAGE SECTION` (e.g.
  `01 COBOL-EVENT-DATA / 05 COBOL-ARRAY-INDEX PIC S9(9) COMP-5.`,
  `PROCEDURE DIVISION USING COBOL-ARRAY-INDEX`), so the developer can identify the
  item that raised the event. *(Exact linkage shape vs. existing event-handler
  generation per §7 Q3.)*
- **R21 (constraint):** the runtime event dispatcher shall track, for each fired
  event: source control id, template control id, repeating-group id, item index,
  and parent-instance id.

### E. Data binding (Phase 5, incremental)
- **R22 (optional):** controls inside a repeating group shall support a
  **BindingPath** (String, default empty) naming the data field used to fill a
  control property; for multi-property controls, property-specific bindings
  (`CaptionBinding`, `TextBinding`, `ValueBinding`, `ImagePathBinding`,
  `ChartDataBinding`) shall be supported.
- **R23 (state):** while a repeating group is bound to a `DataSource`, each
  instance shall receive one data row and the runtime shall fill bound child
  properties automatically from that row.

### F. Cross-cutting constraints
- **R24 (constraint):** every new user-facing IDE string (context-menu items,
  property labels, badge text) shall be a `Tr` field in **all six** languages
  (EN/ES/PT/JA/ZH/FR).
- **R25 (constraint):** the English `docs/developers-guide-en.md` shall document
  the new GroupBox visual properties, repeating-group configuration, indexed
  addressing, shared-handler + index semantics, layout modes, and binding;
  translations untouched.
- **R26 (constraint):** the generated-code banner and the
  regenerate-on-Build/Run/Debug/Check contract shall be unchanged; generated
  handlers for repeating-group controls follow the existing nested-program model
  plus the linkage index.
- **R27 (constraint):** no "cobolt" in user-facing text; COBOL identifiers/source
  stay English.

## 5. Acceptance criteria
*(numbered to match the operator's 15 acceptance points)*

- [ ] **AC1 (R1–R5)** — `GroupBox` exposes HideCaption, HideBackground, corner
  radius, BackgroundColor, and the background-gradient sub-properties; each visibly
  affects rendering at design and run time; defaults leave the look unchanged (R6).
- [ ] **AC2 (R7)** — Right-clicking a `GroupBox` offers "Set as Repeating Group";
  once set, it offers "Unset Repeating Group".
- [ ] **AC3 (R8,R9)** — Repeating-group metadata (all R8 properties) is stored in
  the `.cfrm` on the GroupBox and round-trips on save/reload; children are stored
  once.
- [ ] **AC4 (R12)** — A repeating `GroupBox` with `ItemCount = 7` produces 7 visual
  instances at runtime from the single template.
- [ ] **AC5 (R12,R16)** — Inner controls are repeated as indexed children of each
  instance, addressable as `ArrayName(i)::ChildId`.
- [ ] **AC6 (R13)** — Each instance preserves layout, child sizes, styles,
  containment, z-order (and tab-page ownership if a TabControl is inside).
- [ ] **AC7 (R18)** — A specific inner control uses the **same** handler across all
  instances (one handler, not 7).
- [ ] **AC8 (R19,R20)** — The handler receives the item index through the
  `LINKAGE SECTION` (`COBOL-ARRAY-INDEX`).
- [ ] **AC9 (R19,R20)** — Clicking the button in instance 5 runs the shared handler
  with `COBOL-ARRAY-INDEX = 5`; the developer can branch on it.
- [ ] **AC10 (R14)** — Instances arrange correctly as Vertical, Horizontal, and
  Grid (`ItemsPerRow` + `ItemSpacing`).
- [ ] **AC11 (R15)** — A repeating group inside an `AutoScroll` Panel scrolls when
  instances overflow.
- [ ] **AC12 (R11)** — `PreviewItemCount = 3` previews 3 instances in the designer
  without adding controls to the form model.
- [ ] **AC13 (R6)** — A non-repeating `GroupBox` behaves and looks exactly as before.
- [ ] **AC14 (R18)** — Individual control events **outside** repeating groups keep
  working unchanged (no index linkage forced on them).
- [ ] **AC15 (R22,R23)** — *(Phase 5)* With bindings set and a `DataSource`, each
  instance fills its bound child properties from one data row automatically.
- [ ] **AC16 (R24,R25)** — New UI strings exist in all six languages
  (`cargo test -p cobolt-ide i18n` green); the English guide section lands.

## 6. Constraints & steering check
- **i18n (6 languages):** context-menu items ("Set/Unset as Repeating Group"),
  the Repeating-Group property labels, layout-direction enum labels, gradient
  labels, and the designer badge text are all `Tr` ×6. *(Note: some existing
  property-pane labels use inline literals; new strings follow the steering rule.)*
- **Generated-code / regenerate contract:** generated event handlers for
  repeating-group controls extend the existing nested-program codegen with the
  linkage index; banner + regenerate-on-action behaviour unchanged.
- **Docs:** English guide updated (R25); translations never touched (GOLDEN #3).
- **Reuse over reinvention:** corner-radius, clipping, auto-scroll, opacity, and
  relative-child rendering come from **spec 012**; indexed `::` chains and the
  nested object model from **spec 011**; gradient meshes from **spec 013**. This
  spec must build on those, not duplicate them.
- **Fix vs feature:** substantial new capability → normally a **feature** (minor
  bump). Per the operator's standing directive (product pre-production,
  *everything until further notice is a fix*), treat as a **fix** (patch `z` bump +
  CHANGELOG, forum f=97) unless the directive is lifted before merge.
- **No "cobolt" in user text; COBOL identifiers/source English.**

## 7. Open questions
- **Q1 — corner-radius property name:** spec 012 already added **BorderRadius** to
  GroupBox/Panel/TabControl. Reuse `BorderRadius` (recommended — no duplicate), or
  introduce the `CornerRadius` name the request uses (alias)?
  *Recommendation: reuse `BorderRadius`; surface it in the UI labelled however the
  operator prefers.*
- **Q2 — indexed addressing form:** the request shows
  `CustomerCard(1)::CustomerName::"Caption"`. Spec 011's grammar supports
  `recv(args)::member` chains. Confirm the canonical form is
  `ArrayName(i)::ChildId::Property` (property as a bare member, quoted optional) and
  that the runtime resolves `ArrayName(i)` to the i-th instance object.
- **Q3 — event-handler linkage vs. existing codegen:** today form event handlers
  are generated as nested programs (single source per handler). Does
  `COBOL-ARRAY-INDEX` arrive via `PROCEDURE DIVISION USING …` linkage (request's
  shape), or via a `GET-PROPERTY`/event-data call inside the handler? Linkage is
  cleaner but changes the handler signature only for repeating-group controls —
  confirm whether **all** handlers gain an (ignored) index param or only
  repeating-group ones. *Recommendation: only repeating-group controls get the
  linkage param; non-repeating handlers unchanged (preserves AC14).*
- **Q4 — instance count source of truth:** `ItemCount` (explicit) vs. derived from
  `DataSource` row count when bound. *Recommendation: `DataSource` wins when set,
  else `ItemCount`; expose both.*
- **Q5 — runtime instancing strategy:** materialise N real control objects per
  instance (simplest, matches spec-012 rendering & `::` addressing), vs. a
  virtualised/templated draw that synthesises instances on the fly. *Recommendation:
  materialise N instances into the runtime control tree so existing rendering,
  hit-testing, and `::` addressing "just work"; revisit virtualisation only if
  large `ItemCount` performance demands it.*
- **Q6 — nesting repeating groups:** v1 forbids a repeating group inside another
  repeating group. Confirm this restriction (and whether the designer should block
  marking a GroupBox that contains/contained-by another repeating group).
- **Q7 — phase cut for first merge:** ship **Phases 1–2** (visual props + metadata
  + context menu + designer preview) first, with Phases 3–5 (runtime cloning,
  indexed event dispatch, binding) as follow-on? *Recommendation: yes — Phases 1–2
  are self-contained and testable; 3–5 depend on runtime work and get their own
  plan/tasks.*

## 8. Implementation phases (mirrors the request)
1. **GroupBox visual property expansion** — HideCaption, HideBackground,
   (Border/Corner)Radius clipping, BackgroundColor, BackgroundGradient.
2. **Repeating-group metadata** — IsRepeatingGroup, ArrayName, LayoutDirection,
   ItemSpacing, ItemsPerRow, PreviewItemCount, ItemCount, DataSource,
   AutoScrollParent, CloneEvents; context-menu Set/Unset; designer badge +
   preview.
3. **Runtime cloning** — instantiate N visual instances from the template;
   layout modes; auto-scroll parent.
4. **Indexed event dispatch** — shared handlers + `COBOL-ARRAY-INDEX` via linkage;
   dispatcher resolution (source→template handler + index).
5. **Data binding** — BindingPath / per-property bindings; auto-fill instances
   from a DataSource row.
