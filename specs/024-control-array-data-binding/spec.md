# Spec — Data Binding for Control Arrays (repeating GroupBox)

- **Status:** draft → approved
- **Folder:** specs/024-control-array-data-binding/
- **Author:** Anthropic Claude Codex Agent (with Eslopes)   **Date:** 2026-07-01
- **Builds on:** spec 015 (visual repeating groups — this is its deferred
  Phase 5 "data binding"), spec 022 (Data Binding Guardian — target model,
  guardian gates, generated binding contract), spec 023 (Advanced DataGrid —
  the sibling binding target whose machinery is reused).

## 1. Overview

Let a developer bind a **repeating GroupBox array** (spec 015) to a data source
the same way a `DataGrid` is bound, but with the **array's own controls as the
per-item template** instead of grid columns. One source **row** produces one
**array instance**; each source **field** maps to a **property of one member
control** inside the group (e.g. `CUSTOMER-NAME → NameLabel.Caption`,
`BALANCE → BalanceText.Text`). Binding reuses the existing source editors
(Indexed / SQL / COBOL table / REST / Agent AI), the Data Binding Guardian
gates, the field-mapping model, preview rows, deterministic generated binding
code, and writeback state — only the **target side** differs. A control array is
**only** a legal binding target when its controls belong to a `GroupBox` marked
as a repeating group; nothing outside such a GroupBox can be array-bound.

The target model already exists from spec 022 (`ApprovedBindingTargetKind::
ControlArray`, `BindingTargetDescriptor::ControlArray { array_id,
member_control_ids }`, `BindingTargetPath::ControlProperty { array_id,
control_id, property_name }`). This spec defines the **behaviour**: the binding
editor UI for array targets, field→control-property mapping, data-driven
instance count, read-only and writable population, generated COBOL, guardian
coverage, and the concrete differences from DataGrid binding.

## 2. Goals / Non-goals

### Goals
- Bind a **repeating GroupBox array** to any of the **five** approved sources
  (Indexed file, SQL, COBOL table, REST, Agent AI) — source-side UX identical to
  DataGrid binding.
- Map each **source field → one member control's property** (default property
  per control type, overridable to another writable property of that control).
- **Data-driven instance count**: the number of rendered group instances equals
  the source row count, laid out by the group's spec-015 layout
  (Vertical / Horizontal / Grid) inside an auto-scroll Panel.
- **Read-only and writable** bindings: read-only bindings populate the controls;
  writable bindings (with key fields) push edits made in a member control back to
  the source, reusing the DataGrid/guardian writeback machinery.
- **Guardian coverage**: array bindings are validated and gated on
  save / run / debug / check / build / package exactly like other targets
  (unsafe mappings block the action; repair actions preserve events/layout).
- **Deterministic generated COBOL** that loads the source, populates each
  instance's member properties per mapping, and (writable) writes changed member
  values back — parses, checks, runs, and debugs.
- Reuse the existing binding **field-mapping** model, **preview rows**, and
  **`.cfrm` round-trip** for the new target with no regressions to existing
  bound forms.
- Full **i18n** (6 languages) for all new UI text and **English developer guide**
  coverage of the workflow.

### Non-goals
- New source types beyond the existing five.
- Binding controls that are **not** inside a repeating GroupBox (explicitly
  forbidden — see R5/R6).
- **Nested** repeating-group binding (a bound array inside a bound array) — single
  level only, consistent with spec 015 non-goals. *(Flag, §7 Q4.)*
- DataGrid-only affordances that do not apply to a control template: grid
  columns/headers, per-column COBOL-mask cells, frozen panes, virtual scroll,
  column filters/resize/reorder, CSV export, grid-line styling. (See §8.)
- A general reactive/expression binding engine; mappings stay field→property,
  as in spec 022.
- Changing how a repeating group renders or lays out instances (owned by spec
  015) beyond populating member values.

## 3. User stories
- As a COBOL developer, I want to design a "customer card" GroupBox once, mark it
  as a repeating array, and bind it to a COBOL table so that one card renders per
  table row with each field shown in the right control.
- As a developer, I want each source field to target a specific control's
  property (label caption, textbox text, checkbox checked, picture image) so the
  card looks exactly as I designed it.
- As a developer, I want edits typed into a bound card's TextBox to flow back to
  the source (when the binding is writable and has a key) so the array behaves
  like a writable DataGrid.
- As a developer, I want the IDE to refuse to bind controls that aren't inside a
  repeating GroupBox, and to block unsafe mappings before save/run, so I can't
  corrupt data.
- As a developer, I want the same source editors and preview I already use for
  DataGrids so binding an array feels familiar.

## 4. Requirements (EARS)

**Target eligibility & scope**
- **R1 (ubiquitous):** The system shall treat a `GroupBox` with
  `IsRepeatingGroup = true` as an **approved control-array binding target**,
  whose members are the controls whose `parent` is that GroupBox
  (`explicit_control_array_id` / `binding_target_descriptor_for(id)`).
- **R2 (ubiquitous):** The system shall address a bound array and its members via
  the spec-011 indexed chain `ArrayName(i)::ChildId::Property`, where `i` is the
  1-based instance index.
- **R5 (constraint):** The system shall **not** offer or accept an array binding
  for any control that is not, or whose members are not, inside a repeating
  `GroupBox`; the binding UI for such controls shall expose no array target.
- **R6 (constraint):** While a member control of a repeating GroupBox is
  selected, the system shall expose **only** its array-member mapping
  information (which array + which member), not a standalone scalar binding
  (consistent with spec 022 AC6a).

**Source (shared with DataGrid)**
- **R7 (optional):** Where the user chooses a source for an array target, the
  system shall present the same five source editors as DataGrid binding
  (Indexed / SQL / COBOL table / REST / Agent AI) with identical source-side
  configuration and validation.
- **R8 (ubiquitous):** The system shall reuse the existing binding **field**
  model (name, data type, COBOL mask/PICTURE, edit control) discovered from the
  chosen source.

**Field → control-property mapping**
- **R9 (event):** When the user maps a source field to the array, the system
  shall record a `ControlProperty { array_id, control_id, property_name }`
  target selecting **one member control and one of its properties**.
- **R10 (ubiquitous):** For each member control type, the system shall offer a
  **default bindable property** (e.g. `Label→Caption`, `TextBox→Text`,
  `CheckBox/RadioButton→Checked`, `ComboBox/ListBox→Value`,
  `PictureBox→ImagePath`, `NumericUpDown/ProgressBar/Slider/DateTimePicker→Value`)
  and shall allow overriding it with another writable property of that control.
- **R11 (ubiquitous):** The system shall allow member controls to remain
  **unbound** (static, design-time value) and shall not require every member to
  be mapped.
- **R12 (constraint):** The system shall not allow two source fields to map to the
  **same** `(control_id, property_name)` target within one array binding.

**Population (runtime & preview)**
- **R13 (ubiquitous):** The system shall render **N array instances where N is the
  source row count**, using the group's spec-015 layout (Vertical / Horizontal /
  Grid) inside its auto-scroll Panel.
- **R14 (event):** When a bound form runs (or a target refresh occurs), the
  system shall populate each instance `i`'s mapped member properties from source
  row `i`, formatting values through the field's COBOL mask/PICTURE when defined
  (reusing the DataGrid mask-formatting), and shall leave unmapped member
  properties at their design-time values.
- **R15 (optional):** Where a bound array has more design-capacity than rows (or
  vice versa), the system shall render exactly the row count and shall not leave
  stale instances from a previous data set.
- **R16 (optional):** Where only source **definitions** exist at design time, the
  system shall fill deterministic **preview** instances from the binding fields,
  reusing the DataGrid preview-row logic.

**Writeback (writable bindings)**
- **R17 (optional):** Where an array binding is **writable** and the source
  declares key field(s), the system shall push a changed member property value
  back to the corresponding source field for that instance's row, updating only
  mapped fields and preserving row identity.
- **R18 (state):** While a binding is **read-only**, the system shall never write
  member edits back to the source.
- **R19 (event):** When a writable update fails, the system shall keep the pending
  member value available for repair rather than dropping it (parity with spec 022
  writeback recovery).

**Guardian, generation, persistence (shared)**
- **R20 (event):** When the user saves / runs / debugs / checks / builds /
  packages a form containing an array binding, the Data Binding Guardian shall
  validate the binding and **block** the action on unsafe mappings (missing
  source/target/field, deleted member control, missing writable key, ambiguous
  case-insensitive reference, disallowed conversion) with an actionable message.
- **R21 (ubiquitous):** The system shall generate deterministic COBOL that loads
  the source, populates each instance's mapped member properties, and (writable)
  writes changed member values back; the generated code shall start with the
  developer banner and be regenerated on Build / Run / Debug / Check.
- **R22 (ubiquitous):** The system shall persist array bindings in the `.cfrm`
  with a stable round-trip (save → load identical), reusing the spec-022 binding
  serialization.
- **R23 (event):** When a repair action removes or remaps a member, the system
  shall preserve the group's layout and the member controls' event handlers.

**UX & i18n**
- **R24 (ubiquitous):** The system shall route every new binding-UI string through
  `Tr` translated in all six languages.
- **R25 (ubiquitous):** The system shall document the array-binding workflow and
  its differences from DataGrid binding in the English developer guide.

## 5. Acceptance criteria
- [ ] AC1 — A repeating GroupBox is offered as a bindable **Control Array**
  target; a plain GroupBox and a non-GroupBox control are **not** (R1, R5).
- [ ] AC2 — Selecting a member control shows only its array-member mapping, not a
  scalar binding (R6).
- [ ] AC3 — All five sources are selectable for an array target with the same
  editors/validation as DataGrid (R7, R8).
- [ ] AC4 — A field maps to a chosen member control + property; the default
  property is preselected per control type and can be overridden (R9, R10).
- [ ] AC5 — Two fields cannot target the same member property; the UI/guardian
  rejects it (R12).
- [ ] AC6 — Running a form bound to a source with K rows renders exactly K group
  instances in the group's layout, each member populated from its row and
  formatted through the field mask (R13, R14).
- [ ] AC7 — Unmapped members keep their design-time values; changing the data set
  leaves no stale instances (R11, R14, R15).
- [ ] AC8 — Design-time preview fills deterministic instances from field
  definitions when only definitions exist (R16).
- [ ] AC9 — A writable binding with a key writes a member edit back to the correct
  row/field, updating only mapped fields; a read-only binding never writes back;
  a failed update preserves the pending value (R17, R18, R19).
- [ ] AC10 — Save / run / debug / check / build / package are blocked on unsafe
  array mappings with actionable messages; safe bindings pass (R20).
- [ ] AC11 — Generated COBOL for an array binding parses, checks, runs, and
  debugs; it carries the banner and is regenerated on action (R21).
- [ ] AC12 — Array bindings round-trip through `.cfrm` unchanged; existing
  DataGrid/other bindings are unaffected (R22).
- [ ] AC13 — A repair action that remaps/removes a member preserves layout and
  member event handlers (R23).
- [ ] AC14 — All new binding UI text is localized in the six languages (R24).
- [ ] AC15 — The English developer guide documents the array-binding workflow and
  DataGrid differences (R25).

## 6. Constraints & steering check
- **i18n (6 languages):** New binding-editor labels (target = control array,
  member/property pickers, mapping rows, guardian messages) must be `Tr` fields
  translated in EN/ES/PT/JA/ZH/FR. **Impact: yes.**
- **Generated-code / regenerate contract:** New codegen path for array
  population/writeback; must emit the banner and regenerate on Build/Run/Debug/
  Check. **Impact: yes** (`cobolt-codegen`, `App::regenerate_all_forms`).
- **Docs (English guide):** `docs/developers-guide-en.md` gains an array-binding
  section (and DataGrid-vs-array differences). Translations are user-maintained
  — do not edit. **Impact: yes.**
- **Fix vs feature:** Functionally a **feature** (new binding capability → minor
  bump per tech.md). However the operator's current pre-production standing rule
  treats every change as a **z-level fix**; confirm the version/CHANGELOG
  classification at implementation time. **Flag, §7 Q5.**
- **Reuse constraint:** Must reuse spec-022 target model, guardian, field
  mappings, preview rows, writeback state, and spec-023 mask formatting — no
  parallel binding stack.

## 7. Open questions
- **Q1 (resolved):** Writeback in v1? → **Writable + read-only** (DataGrid
  parity).
- **Q2 (resolved):** Instance count? → **Data-driven, N = source row count**,
  laid out by the group's spec-015 layout.
- **Q3 (resolved):** Sources in v1? → **All five** (Indexed / SQL / COBOL table /
  REST / Agent AI).
- **Q4:** Nested repeating-group binding — confirmed **out of scope** for v1
  (single level), matching spec 015. Confirm no near-term need.
- **Q5:** Version classification at implementation — minor (feature per tech.md)
  vs z (operator's pre-prod "treat all as fixes" rule). Operator to confirm.
- **Q6:** When a writable member is a display-only control type (Label,
  PictureBox), should the guardian treat its field as **read-only-by-nature**
  (no writeback attempted) even inside a writable binding? Proposed: yes —
  writeback applies only to editable member controls (TextBox, ComboBox,
  CheckBox, NumericUpDown, DateTimePicker); resolve before `/plan`.
- **Q7:** Default bindable-property table (R10) — confirm the per-control-type
  default properties and the set of overridable writable properties per type
  during `/plan`.

## 8. DataGrid vs Control-Array binding (shared vs different)

**Shared (reused as-is):** source selection & editors (5 sources); binding field
model (name/type/mask/edit control); Data Binding Guardian gates and repair
actions; field-mapping persistence & `.cfrm` round-trip; preview-row synthesis;
COBOL mask/PICTURE value formatting; deterministic generated-binding contract;
writeback state recovery; i18n & English-guide obligations.

**Different (new for arrays):**

| Aspect | DataGrid | Control Array |
| --- | --- | --- |
| Target unit | Grid **column** | Member **control + property** in a GroupBox |
| Per-item template | Row of cells | The group's designed controls (spec 015) |
| Eligibility | Any `DataGrid` | Only a `GroupBox` with `IsRepeatingGroup` |
| Layout | Grid rows | Vertical / Horizontal / Grid (spec 015) + auto-scroll |
| Instance count | Rows | Rows (same, data-driven) |
| Editing | In-cell editors | The actual member controls |
| Mask | Per-column cell mask | Per-field mask applied to the mapped property |
| Not applicable | headers, frozen panes, virtual scroll, filters, resize/reorder, CSV export, grid-line styling | — |
| Addressing | cell (row, column) | `ArrayName(i)::ChildId::Property` (spec 011) |

## 9. Gate
Review this `spec.md`. When satisfied, run **`/plan`** to design the
implementation (target UI wiring, codegen for population/writeback, guardian
rules, i18n, docs). Do not start design or code from this phase.
