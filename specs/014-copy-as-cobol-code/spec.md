# Spec — Copy as COBOL Code

- **Status:** draft → **SUPERSEDED / ABANDONED** (2026-06-21)
- **Superseded by:** specs/015-visual-repeating-groups/ — the operator chose a
  visual repeating-group (GroupBox-as-array) model instead of emitting
  control-creation COBOL. Do **not** implement this spec.
- **Folder:** specs/014-copy-as-cobol-code/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-06-21

## 1. Overview

PowerRustCOBOL generates a whole form's COBOL as a build artifact, but there is
no way to take **a hand-picked subset of controls** out of the visual designer
and reuse them as **runtime-constructed UI** — the pattern you need for dynamic
panels, repeated row templates, wizard pages, or pasting a designed cluster into
another form's event handler. This feature adds a Form Designer **context-menu
action that serialises the currently selected control(s) into runtime
control-creation COBOL**: a self-contained block that, when executed, recreates
those controls (with their properties, parent/child nesting, and tab order) at
runtime. Coordinates are emitted **relative to the selection anchor** so the
block can be re-placed anywhere, and control identifiers are emitted as
**`WS-` working-storage variables** so the block is template-friendly (the same
block can be instantiated more than once with different names/positions). The
work is staged in three phases: **Phase 1 — Copy as COBOL Code** (clipboard),
**Phase 2 — Copy as COBOL Paragraph** (a callable `PERFORM`-able paragraph), and
**Phase 3 — Save as Runtime Template** (a reusable on-disk template).

This spec covers **Phase 1 in full** and defines the **runtime control-creation
API contract** the emitted code targets. Phases 2–3 are specified at the
requirements level here and detailed in their own plan/tasks later.

## 2. Goals / Non-goals

### Goals
- A Form Designer **context-menu** entry that copies the selected control(s) to
  the clipboard as runtime control-creation COBOL.
- Faithful serialisation: every authored property, the **parent/child** nesting
  (containers from spec 012), and the **tab order** of the selection are
  preserved.
- **Relative coordinates** — positions are emitted relative to the top-left
  anchor of the selection's bounding box, not absolute form coordinates.
- **Template-friendly output** — control names become `WS-`-prefixed
  working-storage identifiers, and the block is delimited by
  `*> BEGIN GENERATED` / `*> END GENERATED` markers so it can be located,
  replaced, or instantiated programmatically.
- A **defined runtime API** (`COBOL-CREATE-CONTROL`, `COBOL-ADD-CHILD`, …) that
  the emitted COBOL calls — so generated blocks are well-formed against a real,
  documented contract (whether that runtime API is *implemented* in this spec or
  a companion spec is an open question, §7 Q1).
- Phases 2 and 3 framed as natural extensions of the Phase 1 serialiser.

### Non-goals
- **Not** a replacement for whole-form codegen (`cobolt-codegen`) — that remains
  the build path for the full `.cfrm`.
- **Not** round-trip import — the feature emits COBOL; parsing emitted COBOL back
  into designer controls is out of scope.
- **Not** an event-handler/logic generator — only control construction +
  property + hierarchy + tab order. Event wiring is out of scope for Phase 1.
- Live data binding, layout managers, and responsive reflow are out of scope.

## 3. User stories
- As a COBOL RAD developer, I want to select a designed cluster of controls and
  copy them as COBOL, so that I can paste a runtime-built version into an event
  handler that creates that UI on demand.
- As a developer building repeated UI (rows, cards, wizard steps), I want the
  copied block to use `WS-` variable names and relative coordinates, so that I
  can instantiate it multiple times at different positions without rewriting it.
- As a developer, I want parent/child nesting and tab order preserved, so the
  runtime-built UI behaves like what I designed.
- As a developer, I want the block fenced with clear BEGIN/END markers, so I (or
  tooling) can find and regenerate it safely.

## 4. Requirements (EARS)

### Phase 1 — Copy as COBOL Code (clipboard)
- **R1 (event):** When one or more controls are selected in the Form Designer
  and the user opens the canvas context menu, the system shall offer a **"Copy as
  COBOL Code"** action.
- **R2 (state):** While no control is selected, the system shall disable (or
  omit) the "Copy as COBOL Code" action.
- **R3 (event):** When the user activates "Copy as COBOL Code", the system shall
  place on the system clipboard a COBOL text block that recreates the selected
  control(s) at runtime.
- **R4 (ubiquitous):** The emitted block shall be delimited by a
  `*> BEGIN GENERATED <id>` line and an `*> END GENERATED <id>` line.
- **R5 (ubiquitous):** For each selected control, the emitted block shall set
  **every authored property** of that control (the same property set the form
  model persists), via the runtime property API.
- **R6 (ubiquitous):** The emitted block shall preserve the **parent/child
  relationships** among the selection: a selected child whose selected ancestor
  is also in the block shall be created as a child of that ancestor.
- **R7 (state):** While a selected control's parent is **not** part of the
  selection, the system shall emit that control as a **top-level** control of the
  block (re-parentable by the consumer), and shall record the original parent
  only as a comment.
- **R8 (ubiquitous):** The emitted block shall preserve the **tab order** of the
  selected controls relative to one another.
- **R9 (ubiquitous):** Control coordinates in the emitted block shall be
  **relative to the top-left anchor** of the selection's bounding box (the
  anchor maps to offset 0,0), so the block is position-independent.
- **R10 (ubiquitous):** Each created control shall be referenced by a
  **`WS-`-prefixed** working-storage identifier derived deterministically from
  the control's name, and identifier collisions within the block shall be
  de-duplicated.
- **R11 (constraint):** The emitted block shall be **valid RustCOBOL** that
  parses without error and targets only the documented runtime
  control-creation/property API (§ R14).
- **R12 (constraint):** The system shall **not** modify the designer document,
  the form model, or any generated `.cbl` when copying (read-only operation).
- **R13 (optional):** Where the selection includes a container with descendants
  that are **not individually selected**, the system shall include those
  descendants in the block (a container copies its whole subtree), so the
  container is reconstructed intact.

### Runtime API contract (targeted by emitted code)
- **R14 (ubiquitous):** The system shall define a runtime control-creation API
  the emitted COBOL targets, comprising at least: **create a control** of a given
  type with a given runtime handle/name, **set a property** on it (existing
  `COBOL-SET-PROPERTY`), **add/attach a child** to a parent container, and **set
  tab order** — named `COBOL-CREATE-CONTROL`, `COBOL-ADD-CHILD`, and
  `COBOL-SET-TAB-ORDER` (final names per plan). *(Implementation of this API is
  scoped per §7 Q1.)*
- **R15 (constraint):** Emitted calls shall reuse the **existing**
  `COBOL-SET-PROPERTY` semantics and property names (spec 010/011) for setting
  control properties, rather than inventing a parallel property vocabulary.

### Phase 2 — Copy as COBOL Paragraph (requirements-level)
- **R16 (event):** When the user activates **"Copy as COBOL Paragraph"**, the
  system shall emit the Phase-1 block wrapped as a named, `PERFORM`-able COBOL
  **paragraph** (with the same BEGIN/END markers), so it can be pasted into a
  PROCEDURE DIVISION and invoked by name.

### Phase 3 — Save as Runtime Template (requirements-level)
- **R17 (event):** When the user activates **"Save as Runtime Template"**, the
  system shall persist the serialised block as a **reusable named template** on
  disk within the project, so it can be re-inserted/instantiated later.

## 5. Acceptance criteria

### Phase 1
- [ ] AC1 — With ≥1 control selected, the canvas context menu shows "Copy as
  COBOL Code"; with nothing selected it is disabled/absent. (R1, R2)
- [ ] AC2 — Activating it puts text on the clipboard fenced by
  `*> BEGIN GENERATED …` / `*> END GENERATED …`. (R3, R4)
- [ ] AC3 — For a single control, the clipboard block creates that control and
  sets every property the form model holds for it; round-tripping the property
  set against the model shows no authored property omitted. (R5)
- [ ] AC4 — Selecting a container plus children (spec 012) yields a block in
  which children are added to their parent; selecting a child whose parent is not
  selected yields a top-level control with the original parent noted in a
  comment. (R6, R7, R13)
- [ ] AC5 — The relative order of `COBOL-SET-TAB-ORDER` (or equivalent) matches
  the designer tab order of the selection. (R8)
- [ ] AC6 — Coordinates in the block are relative: the top-left-most control sits
  at (0,0) and others at their offsets from it; no absolute form coordinate
  appears. (R9)
- [ ] AC7 — Control handles are `WS-`-prefixed, deterministic from the control
  name, and unique within the block. (R10)
- [ ] AC8 — The emitted block **parses without error** through the RustCOBOL
  parser and references only documented runtime CALLs. (R11, R14, R15)
- [ ] AC9 — Copying leaves the designer document/model and any generated `.cbl`
  byte-for-byte unchanged. (R12)

### Runtime API (verification depends on §7 Q1 resolution)
- [ ] AC10 — The runtime control-creation API names + signatures are documented
  in the English developers guide, and (if in scope per Q1) a generated block
  executed by the runtime visibly recreates the selected controls. (R14, R15)

### Phases 2–3 (deferred verification)
- [ ] AC11 — Phase 2: "Copy as COBOL Paragraph" emits a valid, `PERFORM`-able
  paragraph wrapping the Phase-1 block. (R16)
- [ ] AC12 — Phase 3: "Save as Runtime Template" writes a reusable named template
  into the project. (R17)

## 6. Constraints & steering check

- **i18n (6 languages):** New context-menu labels ("Copy as COBOL Code",
  "Copy as COBOL Paragraph", "Save as Runtime Template") and any toast/status
  strings are **`Tr` fields translated in all six** (EN/ES/PT/JA/ZH/FR) in
  `crates/cobolt-ide/src/i18n.rs`. No hard-coded UI literals.
- **Generated-code / regenerate contract:** This emits an **ad-hoc clipboard
  artifact**, not a managed `.cbl`, so it is **outside** the
  regenerate-on-Build/Run/Debug contract. It must **not** be written into or
  confused with `cobolt-codegen` output, and copying must not trigger form
  regeneration. The block still carries a generated-code marker
  (`*> BEGIN/END GENERATED`) so it is recognisable as machine-emitted.
- **Voice/branding:** Emitted COBOL and comments use English identifiers and must
  **never** contain the internal "cobolt" prefix in user-facing text (the public
  CALL names are `COBOL-…`, consistent with existing built-ins).
- **Docs (English guide only):** `docs/developers-guide-en.md` gains a section
  documenting the feature and the runtime control-creation CALL contract.
  Translations are **user-maintained — not edited** (GOLDEN #3).
- **Fix vs feature:** This is new user-visible functionality. Per the operator's
  standing directive (product not production-ready, *everything until further
  notice is a fix*), it is treated as a **fix**: bump the **patch** (`z`) in
  `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md`, and publish to forum
  **f=97** (no prefix). If that directive is lifted before merge, reclassify as a
  feature (minor `y`, f=96 `[Noticia]`).

## 7. Open questions

- **Q1 — Runtime API: implement now or companion spec?** Phase 1 *emits* COBOL
  that calls `COBOL-CREATE-CONTROL` / `COBOL-ADD-CHILD` / `COBOL-SET-TAB-ORDER`,
  which **do not exist** in the runtime today (confirmed: runtime exposes
  `COBOL-INIT-FORM`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`,
  `COBOL-WAIT-EVENT`, chart/file/HTTP/SQL CALLs — but no control-creation CALLs).
  Two options:
  **(A)** Implement the runtime API in this spec so copied blocks actually
  **run** (larger scope: runtime + IDE preview must support dynamic control
  creation, mutating the live form model at runtime). AC10's executable check
  applies.
  **(B)** Scope this spec to the **serialiser only** — emit well-formed COBOL
  against the *defined* contract; defer the runtime implementation to a companion
  spec (015). AC10 reduces to "API documented; emitted code parses". *(Default
  assumption pending your call: **(B)** — ship the copy/serialise feature against
  a documented contract, implement the runtime API next.)*
- **Q2 — Relative-coordinate anchor:** confirm the anchor is the **bounding-box
  top-left of the whole selection** (default), vs. each control relative to its
  own parent's content origin. *(Default: selection bounding-box top-left;
  children inside a copied container are relative to that container's content
  origin, mirroring spec 012 containment.)*
- **Q3 — Property breadth:** emit **all** persisted properties (default,
  faithful but verbose) or only **non-default** ones (compact, but couples output
  to default tables)? *(Default: all persisted properties, matching what the
  `.cfrm` stores.)*
- **Q4 — Phase split:** is shipping **Phase 1 alone** acceptable for the first
  merge, with Phases 2–3 as follow-on specs/tasks? *(Default: yes.)*
