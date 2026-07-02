# Spec — Data binding guardian and source wiring

- **Status:** draft
- **Folder:** specs/022-data-binding-guardian/
- **Author:** Claude (spec-driven)   **Date:** 2026-06-29

## 1. Overview

Add a guarded data-binding workflow for visual controls that display or edit
records. The IDE shall let a developer choose a data source type, map source
fields to target controls, validate whether the target is suitable, and generate
safe COBOL/runtime wiring without breaking existing data-bound behavior. A new
Data Binding Guardian shall act as a deterministic local validator for binding
changes and block or warn on changes that could corrupt reads, display stale
values, lose user edits, or desynchronise control state from source data.

This feature covers binding from Indexed files, SQL, COBOL tables, REST APIs,
and Agent AI outputs into a deliberately small target set: charts, dropdowns
(`ComboBox`), `ListBox`, `DataGrid`, and explicit arrays of controls. Standalone
scalar controls do not expose data-binding information; scalar binding fields
are visible only when the scalar control belongs to an explicit control array.
It is a RAD workflow and safety layer; it must preserve the existing
generated-code contract and form runtime behavior.

## 2. Goals / Non-goals

### Goals

- Provide a first-class data-binding configuration for controls that display or
  modify structured data.
- Support source selection for:
  - Indexed files (`.cidx` definitions and indexed runtime files).
  - SQL (`SqlDatabase` controls and query/result-set fields).
  - COBOL tables/arrays declared in generated or user COBOL.
  - REST API responses (`RestClient` controls and response data items).
  - Agent AI responses (`AgentObject` outputs that can produce structured data).
- Let the developer map source fields to chart series/categories, dropdown/list
  items, grid columns, or target properties inside an explicit control array.
- Support binding targets only for charts, dropdowns (`ComboBox`), `ListBox`,
  `DataGrid`, and explicit arrays of controls.
- Hide data-binding information for standalone scalar controls.
- Expose scalar-control data-binding information only through an explicit
  control-array binding context.
- Block bindings to any control type outside the approved target allow-list.
- Add a Data Binding Guardian validator that checks binding edits before they are
  accepted, saved, generated, run, debugged, or built.
- Keep existing `DataItem`, `DataFormat`, `DataSource`, `Rows`, `Columns`,
  `RequestDataItem`, `ResponseDataItem`, `ResultSetDataItem`, and
  `TargetControls` behavior compatible.
- Ensure generated code remains parseable, semantically valid, executable by the
  runtime, and debuggable in the IDE.

### Non-goals

- Replacing the existing Indexed File Editor, SQL, REST, AgentObject, or
  DataGrid controls.
- Adding a new external database driver or network stack.
- Designing a full ORM.
- Live cloud sync of data source schemas.
- Automatic destructive migrations for SQL tables, indexed files, or COBOL
  table layouts.
- Allowing Agent AI to modify arbitrary controls outside the binding contract.

## 3. User stories

- As a COBOL developer, I want to bind an Indexed file to a grid so I can browse
  and edit records visually without hand-writing all field copy logic.
- As a COBOL developer, I want to bind a SQL query result to repeated User
  Controls or a chart so I can build business screens from database rows.
- As a COBOL developer, I want to bind a COBOL table to a group of TextBoxes and
  ComboBoxes so a form can display and edit an in-memory array.
- As a COBOL developer, I want to bind a REST response to a grid or repeated
  controls so remote business data can be displayed consistently.
- As a COBOL developer, I want to bind structured Agent AI output only to
  approved target controls so AI-driven updates cannot accidentally overwrite
  unrelated form state.
- As a COBOL developer, I want data-binding fields hidden on standalone scalar
  controls, so only valid grid or array-based targets can be wired to data.
- As a project maintainer, I want a guardian validator to block risky binding
  changes so future edits do not silently break read/display/update semantics.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall represent each data binding as a
  structured top-level form binding definition that references one approved
  binding target.

- **R1a (constraint):** Binding definitions shall include a binding schema
  version, stable binding id, source descriptor, target descriptor, ordered field
  mappings, mode (`ReadOnly` or `Writable`), saved validation state, and saved
  source schema/sample metadata where applicable.

- **R2 (ubiquitous):** The system shall support these binding source types:
  `IndexedFile`, `Sql`, `CobolTable`, `RestApi`, and `AgentAi`.

- **R3 (ubiquitous):** The system shall store enough source metadata to identify
  the chosen source, including source type, source control or project resource,
  record/table/query/endpoint/output name, field list, key field when available,
  and read/write capability.

- **R4 (ubiquitous):** The system shall let the developer map each source field
  to chart series/category fields, dropdown/list item fields, grid columns, or a
  target control property within an explicit target control array.

- **R5 (constraint):** The system shall expose data-binding information only for
  approved binding targets: chart controls, dropdowns (`ComboBox`), `ListBox`,
  `DataGrid`, and explicit arrays of controls.

- **R5a (constraint):** The system shall treat `DataGrid` as the canonical valid
  row-shaped grid target. `GridControl` shall be accepted only as a user-facing
  alias for `DataGrid` if such an alias already exists in the IDE.

- **R5b (constraint):** The system shall treat chart controls as valid binding
  targets only for chart-shaped mappings, such as category labels, value series,
  and optional series names.

- **R5c (constraint):** The system shall treat dropdowns (`ComboBox`) and
  `ListBox` controls as valid binding targets only for item-list mappings and
  selection/value mappings.

- **R6 (constraint):** The system shall treat a target control array as valid
  only when it has an explicit repeat/array contract, such as a repeating
  GroupBox/User Control instance collection, or an equivalent binding collection
  defined by the form model.

- **R6a (constraint):** The IDE shall hide data-binding information for all
  controls that are not approved binding targets, including standalone scalar
  controls.

- **R6b (constraint):** The IDE shall hide standalone source selection for scalar
  controls even when they belong to an explicit control array; array-member
  scalar controls may expose only field-mapping information owned by the array
  binding.

- **R7 (event):** When the developer selects a row-shaped source and a target
  that is not an approved binding target for row-shaped data, the IDE shall
  block the binding.

- **R8 (event):** When the developer selects a scalar control that belongs to a
  valid control array, the IDE shall expose only the field-mapping information
  relevant to that array binding and shall not expose standalone source
  selection on the scalar control.

- **R9 (state):** While a binding is read-only, the runtime and generated code
  shall not write user edits back to the source.

- **R10 (state):** While a binding is writable, the runtime and generated code
  shall preserve source keys or row identity needed to update the correct source
  record.

- **R10a (state):** During initial source load, the system shall not mark mapped
  controls dirty and shall not write loaded values back to the source.

- **R10b (event):** When a user edits a writable bound target, the system shall
  update pending binding state only; source writes shall occur only through an
  explicit save/update action or an existing form event contract.

- **R10c (event):** When a writable source update fails, the system shall leave
  pending user edits recoverable and report the failure without clearing row
  identity.

- **R11 (event):** When source data is loaded, the system shall populate mapped
  target controls without replacing unrelated properties, events, styles,
  containment, z-order, or user-control qualification.

- **R12 (event):** When a bound target control value changes, the system shall
  update the binding's pending data state without corrupting unmapped source
  fields.

- **R13 (event):** When the developer saves a form, runs a form, debugs a form,
  checks a project, builds a project, or packages a project, the Data Binding
  Guardian shall validate all binding definitions before the action proceeds.

- **R14 (constraint):** The Data Binding Guardian shall block changes that would
  break existing bound behavior, including missing source fields, incompatible
  target cardinality, missing row identity for writable bindings, unsafe Agent AI
  target scope, and stale mappings to deleted controls.

- **R15 (constraint):** The Data Binding Guardian shall warn, but not necessarily
  block, when a binding can run but may lose fidelity, such as type coercion,
  read-only fallback, nullable-to-required field mapping, or partial REST/Agent
  schema inference.

- **R15a (constraint):** The Data Binding Guardian shall classify mapping
  compatibility as `Exact`, `CoercibleWarning`, or `Blocked` using deterministic
  rules for numeric truncation, date reformatting, nullable-to-required mapping,
  multi-value-to-scalar mapping, and unknown REST/Agent field types.

- **R16 (constraint):** The Data Binding Guardian shall report findings in the
  IDE with stable severity levels: `Blocker`, `Warning`, and `Info`.

- **R17 (ubiquitous):** The binding editor shall expose source-specific wiring:
  - Indexed files: choose a project `.cidx` definition and map record fields.
  - SQL: choose a `SqlDatabase` control/query/result set and map columns.
  - COBOL table: choose a COBOL table/array item and map subfields.
  - REST API: choose a `RestClient` response data item/schema and map JSON fields.
  - Agent AI: choose an `AgentObject` structured response and approved target
    controls.

- **R18 (constraint):** Agent AI bindings shall only write to controls included
  in the binding's approved target list and shall never infer additional target
  controls at runtime.

- **R18a (constraint):** REST API and Agent AI bindings shall be read-only unless
  explicit update metadata is present, including update request schema, key/row
  identity mapping, and approved target scope. The Guardian shall block writable
  REST or Agent AI bindings that lack this metadata.

- **R19 (constraint):** Binding source and target references shall resolve
  case-insensitively where they refer to COBOL/control identifiers, while
  preserving designed casing in saved `.cfrm` data.

- **R20 (constraint):** Existing forms without binding definitions shall load,
  render, preview, run, debug, build, and save without behavior changes.

- **R20a (constraint):** Existing `.cfrm` files without binding metadata shall
  deserialize with an empty binding list, serialize without changing unrelated
  control XML, and remain loadable by the current form, render, and codegen
  paths.

- **R21 (constraint):** Generated COBOL for bindings shall remain deterministic,
  parseable by `cobolt-parser`, valid under `cobolt-semantic`, executable by
  `cobolt-runtime`, and compatible with debugger source mapping.

- **R21a (constraint):** Binding definitions, field mappings, Guardian findings,
  and generated binding code shall use deterministic ordering so tests and
  regenerated output are stable.

- **R22 (constraint):** Binding validation shall not require network access for
  REST or Agent AI sources; when live schemas are unavailable, the IDE shall use
  saved schemas, sample payloads, response data item structure, or explicit field
  mappings.

- **R23 (event):** When a source schema changes, the IDE shall detect stale
  mappings where possible and surface Guardian findings before generation or
  execution.

- **R24 (ubiquitous):** The system shall provide a clear path to refresh or
  repair a stale binding without deleting existing event handlers or visual
  control layout.

- **R24a (ubiquitous):** Binding repair actions shall include remapping a missing
  field, removing a mapping, marking a binding read-only, refreshing from saved
  schema/sample metadata, refreshing from an available project source, and
  reselecting a target control.

- **R25 (constraint):** The Data Binding Guardian shall detect case-insensitive
  source or target collisions, such as two controls whose identifiers differ
  only by case, and block bindings where resolution would be ambiguous.

- **R26 (constraint):** The Data Binding Guardian shall block any binding
  definition that references a target outside the approved allow-list: chart
  controls, dropdowns (`ComboBox`), `ListBox`, `DataGrid`, and explicit arrays
  of controls.

## 5. Acceptance criteria

- [ ] AC1 — A form can define a binding from an Indexed file definition to a
  `DataGrid`, mapping at least two record fields to grid columns.
- [ ] AC2 — A form can define a binding from a SQL result set to a repeating
  User Control/control array, mapping at least two columns to child controls.
- [ ] AC3 — A form can define a binding from a COBOL table/array to TextBox and
  ComboBox/ListBox controls in an explicit control array.
- [ ] AC4 — A form can define a binding from a REST response to a `DataGrid`
  without requiring live network access during validation.
- [ ] AC5 — A form can define a binding from an Agent AI structured response only
  to approved target controls.
- [ ] AC5a — A form can define a binding from a supported source to a chart,
  mapping at least one category field and one value series.
- [ ] AC5b — A form can define a binding from a supported source to a dropdown
  (`ComboBox`) and a `ListBox`, mapping display item text and selected value.
- [ ] AC6 — Controls outside the approved allow-list do not show data-binding
  information in the Properties panel or binding editor.
- [ ] AC6a — Scalar controls that belong to an explicit control array expose
  only the mapping information for that array binding, not standalone source
  selection.
- [ ] AC7 — Guardian validation blocks a writable binding whose source key/row
  identity is missing.
- [ ] AC8 — Guardian validation blocks a binding that references a deleted
  source, deleted target control, or missing mapped field.
- [ ] AC9 — Guardian validation warns on lossy type conversions but allows the
  developer to continue when the binding remains executable.
- [ ] AC10 — Running a bound form populates target controls from the selected
  source without altering unmapped visual properties or event handlers.
- [ ] AC11 — Editing a writable bound target updates only mapped fields and
  preserves unmapped source fields.
- [ ] AC12 — Existing unbound forms round-trip through load/save and Run Form
  with no binding warnings and no behavior changes.
- [ ] AC13 — For every source type included in the implementation phase,
  generated binding COBOL parses, passes semantic analysis, runs through the live
  interpreter, and does not break debugger source mapping.
- [ ] AC14 — Binding-related user-facing IDE text is localized through `Tr` in
  all six supported languages.
- [ ] AC15 — The English developer guide documents the binding workflow and the
  Guardian severities; translated guides are not edited.
- [ ] AC16 — A form with each implemented binding source type round-trips
  through `.cfrm` save/load with stable binding metadata, preserved designed
  casing, and unchanged unrelated control properties, events, and layout.
- [ ] AC17 — Debugging generated binding code preserves breakpoint/step behavior
  and source-line mapping for generated binding sections and user event handlers.
- [ ] AC18 — REST and Agent AI bindings validate from saved schemas, samples, or
  explicit mappings without network access.
- [ ] AC19 — Guardian validation blocks ambiguous case-insensitive source or
  target references.
- [ ] AC20 — Repair actions can remap a missing field, remove a mapping, mark a
  binding read-only, refresh from schema/sample metadata, and reselect a target
  control without deleting event handlers or visual layout.

## 6. Constraints & steering check

- **i18n impact:** Yes. The binding editor, source-type labels, warnings,
  Guardian severities, repair actions, and validation messages must be `Tr`
  fields translated in EN/ES/PT/JA/ZH/FR.
- **Generated-code / regenerate contract impact:** Yes. Binding code must be
  emitted deterministically by `cobolt-codegen` and regenerated on Build / Run /
  Debug / Check. Generated `.cbl` remains a build artifact.
- **Docs update needed:** Yes. Update only `docs/developers-guide-en.md`.
- **Fix vs feature classification:** In normal semver this is a feature, but the
  repo convention currently treats every change as a `z` bump until told
  otherwise.
- **Crate boundaries:** Form binding metadata belongs in `cobolt-forms`;
  generated binding COBOL belongs in `cobolt-codegen`; runtime read/write
  behavior belongs in `cobolt-runtime`; IDE binding editor and Guardian UI belong
  in `cobolt-ide`; Indexed source metadata should reuse the existing
  indexed-file model/editor types where present without adding a new crate unless
  explicitly planned.
- **Backward compatibility:** Existing `.cfrm`, `.cidx`, generated COBOL, and
  unbound control properties must remain compatible.
- **Network safety:** Validation must not depend on live REST/Agent endpoints.
- **Agent safety:** Agent AI bindings must be scope-limited to explicit targets
  and guarded before generation and execution.

## 7. Data Binding Guardian charter

The Data Binding Guardian is a specialized project validation role implemented
as deterministic local checks inside PowerRustCOBOL. It exists to protect
data-bound controls from regressions in data access, display, and modification
behavior. It shall not require network access or external AI calls.

The Guardian shall:

- Treat existing working bindings as protected contracts.
- Review binding metadata, source schemas, target controls, generated COBOL, and
  runtime update paths for breakage risk.
- Block unsafe changes before save/run/debug/check/build/package.
- Prefer targeted warnings for recoverable risks.
- Explain each finding in terms a COBOL RAD developer can act on.
- Never silently rewrite bindings without an explicit user action.

The Guardian shall block:

- Writable bindings without stable row identity.
- Mappings to deleted controls, deleted source fields, or deleted source
  resources.
- Any binding that targets a control outside the approved allow-list: chart
  controls, dropdowns (`ComboBox`), `ListBox`, `DataGrid`, and explicit arrays
  of controls.
- Agent AI bindings that can write outside their approved target controls.
- Generated binding code that fails parser or semantic validation.
- Ambiguous case-insensitive source or target resolution.
- Writable REST or Agent AI bindings without explicit update metadata.

The Guardian shall warn:

- Type conversions that may truncate or reformat data.
- REST or Agent schemas inferred from samples.
- Read-only fallback for sources that cannot be safely updated.
- Partial mappings that intentionally ignore source fields.
- Bindings whose source schema changed since the last validation.

## 8. Open questions

- Q1: Should the first implementation include all five source types, or should
  `/plan` phase split them into an MVP source subset plus follow-up tasks?
- Q2: What user-facing term should be used for the target array concept:
  "control array", "repeating controls", or "bound collection"?
- Q3: Which explicit user action should commit pending writable binding edits:
  a generated Save button/paragraph, an event-handler call, or both?
