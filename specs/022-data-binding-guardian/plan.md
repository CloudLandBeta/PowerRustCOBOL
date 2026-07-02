# Plan — Data binding guardian and source wiring

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-29

## 1. Approach

Implement data binding as an explicit form-level contract, not as standalone
per-control scalar properties. Each binding definition will own its source
descriptor, approved target descriptor, ordered field mappings, mode, validation
snapshot, and saved source metadata (R1-R4). This keeps scalar controls from
advertising source selection while still allowing scalar fields inside explicit
control arrays to participate through their parent array binding (R5-R8).

Add a deterministic Data Binding Guardian module that validates binding
definitions in both edit-time and action-gate paths. The Guardian will classify
findings as `Blocker`, `Warning`, or `Info`, with mapping compatibility of
`Exact`, `CoercibleWarning`, or `Blocked` (R13-R16, R25-R26). Save, run, debug,
check, build, and package actions will call the same validator before proceeding
so the IDE cannot persist or execute unsafe binding edits through a side path.

Add an IDE binding editor reachable only from approved targets: chart controls,
`ComboBox`, `ListBox`, `DataGrid`, and explicit control arrays such as repeating
groups or User Control instance collections (R5-R6). The editor will expose
source-specific selectors for Indexed files, SQL result sets, COBOL tables, REST
responses, and Agent AI structured outputs, then present mapping surfaces that
match the selected target shape: grid columns, chart category/value series,
dropdown/list item fields, or array-member target properties (R17-R18a).

Generate deterministic COBOL/runtime wiring from validated bindings only. Load
paths must populate mapped target state without marking controls dirty, replacing
unmapped properties, changing events, or changing layout metadata (R9-R12,
R21-R21a). Writable bindings keep row identity and pending edits separate from
source data until an explicit save/update action or existing event contract
commits them (R10-R10c). REST and Agent AI remain read-only unless explicit
update metadata is present (R18a, R22).

Preserve existing forms by treating missing binding metadata as an empty binding
list and by leaving existing control properties serialized as they are today
(R20-R20a). Legacy scalar `DataItem`/`DataFormat` values may still round-trip in
`.cfrm`, but the IDE property panel will no longer expose standalone
data-binding UI for scalar controls outside an approved array mapping context.

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs` — add form-level binding structs, source
  and target enums, mapping structs, validation snapshot structs, compatibility
  enums, and helper methods that answer whether a control is an approved binding
  target.
- `crates/cobolt-forms/src/xml.rs` or the existing `.cfrm` persistence module —
  persist an optional top-level `DataBindings` section, deserialize missing
  sections as empty, and preserve unrelated control XML.
- `crates/cobolt-forms/src/lib.rs` — expose the binding model and Guardian-facing
  helpers without leaking IDE-specific concepts into the forms crate.
- `crates/cobolt-ide/src/panels/properties.rs` — remove the universal
  data-binding property section for scalar controls; show binding affordances
  only for approved targets and array-owned mapping context.
- `crates/cobolt-ide/src/panels/data_binding.rs` — add the binding editor UI for
  source selection, target mapping, stale mapping repair, validation results,
  and read-only/writable mode controls.
- `crates/cobolt-ide/src/data_binding_guardian.rs` — implement deterministic
  binding validation, blocker/warning/info reporting, allow-list enforcement,
  case-insensitive reference resolution, stale mapping detection, and repair
  recommendations.
- `crates/cobolt-ide/src/app.rs` and form/project action handlers — gate save,
  run, debug, check, build, and package actions through the Guardian.
- `crates/cobolt-ide/src/i18n.rs` — add all binding editor, Guardian finding,
  warning, and repair-action strings in the six supported languages.
- `crates/cobolt-codegen/src/lib.rs` and/or a new
  `crates/cobolt-codegen/src/data_binding.rs` — emit deterministic binding
  initialization, load/populate, pending edit, and update stubs using only
  syntax supported by the parser/runtime.
- `crates/cobolt-runtime/src/interpreter.rs` and existing form/runtime bridge
  modules — support generated binding helper calls where current control state
  updates are insufficient, while keeping UI-specific behavior in the IDE.
- `crates/cobolt-runtime` source adapter modules for Indexed, SQL, REST, and
  AgentObject behavior — reuse existing built-ins and add only the minimal
  adapter surface needed by generated binding code.
- `crates/cobolt-semantic` — add validation only if generated binding helper
  calls introduce new named built-ins or generated data-item patterns.
- `docs/developers-guide-en.md` — document the binding workflow, approved target
  list, scalar-control visibility rules, Guardian severities, repair workflow,
  and source-specific wiring.
- `CHANGELOG.md` and version metadata — add the required z-version bump and
  changelog entry during implementation.

## 3. Data / model changes

Add optional form metadata:

```text
Form
  data_bindings: Vec<DataBindingDef>

DataBindingDef
  schema_version: u16
  id: String
  display_name: String
  source: BindingSourceDescriptor
  target: BindingTargetDescriptor
  mappings: Vec<FieldMapping>
  mode: BindingMode
  validation: BindingValidationSnapshot
  saved_source_metadata: BindingSourceMetadata
```

`BindingSourceDescriptor` will cover `IndexedFile`, `Sql`, `CobolTable`,
`RestApi`, and `AgentAi`. Each variant stores source type, project resource or
source control reference, record/table/query/endpoint/output name, fields,
optional key fields, read/write capability, and saved schema/sample metadata.

`BindingTargetDescriptor` will cover `DataGrid`, `Chart`, `ComboBox`, `ListBox`,
and `ControlArray`. Chart mappings include category and one or more value
series. Dropdown/list mappings include display item and optional value/selection
fields. Grid mappings target stable column ids. Control-array mappings target a
child control id plus a property path, and are valid only when the parent has an
explicit repeat/array contract.

`.cfrm` files gain an optional top-level `DataBindings` section with deterministic
binding and mapping order. Missing sections deserialize as `Vec::new()`. Existing
control properties, including legacy scalar `DataItem` and `DataFormat`, remain
round-trippable for compatibility, but new binding behavior is driven by the
top-level binding list.

Reference resolution is case-insensitive for COBOL/control identifiers while
preserving the designed casing in saved XML. The Guardian blocks ambiguous
case-insensitive collisions instead of guessing.

## 4. Key decisions & alternatives

- Decision: store bindings at form level. Why: the source-target-mapping contract
  is cross-control state and must be validated as a unit. Rejected: extending
  every scalar control with source fields, because that violates the approved
  target allow-list and makes unsafe standalone scalar binding easy.
- Decision: keep legacy scalar properties serialized but hidden from the normal
  property panel. Why: this protects old `.cfrm` round-trips while enforcing the
  new visibility rule. Rejected: deleting or migrating legacy values on load,
  because that risks data loss.
- Decision: implement the Guardian as deterministic local Rust logic. Why:
  validation must run offline, in tests, and before build/package actions.
  Rejected: a remote or prompt-driven validator, because validation must be
  reproducible and cannot require network access.
- Decision: use the current visual target names, with `DataGrid` as canonical.
  Why: the codebase already models `DataGrid`; `GridControl` is accepted only as
  a user-facing alias if the IDE already exposes that term. Rejected: introducing
  a separate grid widget type.
- Decision: source support shares one schema but can be implemented behind
  source-specific adapters. Why: Indexed, SQL, COBOL table, REST, and Agent AI
  have different metadata and write semantics, but the mapping/validation UI
  must be consistent. Rejected: five unrelated binding systems.
- Decision: REST and Agent AI default to read-only. Why: safe writable behavior
  requires explicit update metadata, key identity, and approved target scope.
  Rejected: inferring writeback behavior from a response payload.

## 5. Risks & mitigations

- Risk: the feature crosses model, IDE, codegen, runtime, and docs. Mitigation:
  implement in layers: model/XML and allow-list first, Guardian next, IDE editor
  next, then codegen/runtime source adapters.
- Risk: hiding scalar binding UI could appear to remove old data. Mitigation:
  preserve existing serialized properties, hide them only in the editor, and add
  regression tests proving old forms round-trip without unrelated XML changes.
- Risk: generated binding code may drift beyond parser/runtime support.
  Mitigation: generate through focused codegen tests that immediately parse,
  semantically validate, and execute the supported helper flow.
- Risk: writeback can corrupt records if row identity is lost. Mitigation: block
  writable bindings unless identity fields are mapped and retained in pending
  state; keep read-only fallback as a warning where safe.
- Risk: REST/Agent schema validation could accidentally depend on live network
  calls. Mitigation: validate only against saved schemas, samples, explicit
  mappings, or local response data-item structures.
- Risk: case-insensitive identifier matching can be ambiguous. Mitigation: build
  a normalized reference index and block collisions before mappings are saved.

## 6. Test strategy

- `cobolt-forms` unit tests:
  - approved target classification for charts, `ComboBox`, `ListBox`,
    `DataGrid`, and explicit arrays;
  - rejected target classification for standalone scalar controls;
  - `.cfrm` round-trip for forms with and without `DataBindings`;
  - deterministic binding/mapping serialization order;
  - legacy scalar `DataItem`/`DataFormat` round-trip without creating a new
    binding definition.
- `cobolt-ide` unit tests:
  - property panel visibility hides standalone scalar data-binding information;
  - array-member scalar controls expose only array-owned field mapping context;
  - Guardian blocks unsupported targets, stale deleted controls, missing fields,
    missing writable identity, unsafe Agent target scope, and case collisions;
  - Guardian warns for coercible type mappings, nullable-to-required mappings,
    read-only fallback, and partial REST/Agent schema inference.
- `cobolt-codegen` tests:
  - generated Indexed-to-`DataGrid`, SQL-to-array, COBOL-table-to-array,
    REST-to-`DataGrid`, Agent-to-approved-target, chart, `ComboBox`, and
    `ListBox` bindings are deterministic and parseable;
  - generated code preserves unmapped properties/events and does not mark
    controls dirty during initial load.
- `cobolt-runtime` tests:
  - read-only bindings never write back;
  - writable bindings preserve row identity and pending edits until explicit
    update;
  - failed updates leave pending edits recoverable;
  - mapped target updates do not corrupt unmapped source fields.
- Integration/manual verification:
  - launch the IDE, create each supported binding source/target combination,
    confirm Guardian findings appear with localized text, and confirm save/run
    gates block only blocker findings;
  - verify existing forms without binding metadata still open, preview, run,
    save, and regenerate without unrelated changes.

## 7. Steering compliance

- [ ] i18n: all new UI strings in 6 languages.
- [ ] Generated-code banner + regenerate-on-action contract preserved.
- [ ] English developer guide updated; translations untouched.
- [ ] Feature: z-version bump and 2026-06-29 changelog entry during
  implementation.
- [ ] No "cobolt" in user-facing text; COBOL identifiers remain English.
