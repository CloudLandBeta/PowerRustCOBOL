# Tasks — Data binding guardian and source wiring

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-06-29

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

---

## Layer 1: Form model and persistence

- [x] **T1 — Form-level binding model** (R1, R1a, R2, R3, R4,
  R21a)
  - Files: `crates/cobolt-forms/src/model.rs`,
    `crates/cobolt-forms/src/lib.rs`
  - Do: Add `DataBindingDef`, source descriptors for `IndexedFile`, `Sql`,
    `CobolTable`, `RestApi`, and `AgentAi`, target descriptors for `DataGrid`,
    chart, `ComboBox`, `ListBox`, and `ControlArray`, ordered field mappings,
    binding modes, saved source metadata, validation snapshots, Guardian finding
    types, and mapping compatibility enums. Add deterministic constructors and
    ordering helpers.
  - Verify: `cargo test -p cobolt-forms data_binding_model --features render`
    passes. Tests assert all source/target variants serialize through the Rust
    model and keep mapping order stable. **Covers AC1, AC2, AC3, AC4, AC5,
    AC5a, AC5b, AC16.**

- [x] **T2 — Approved target classification helpers** (R5, R5a, R5b,
  R5c, R6, R6a, R6b, R26)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: Add helpers that classify chart controls, `ComboBox`, `ListBox`,
    `DataGrid`, and explicit control arrays as valid binding targets. Treat
    scalar controls as invalid unless they are resolved through an owning
    control-array mapping. Treat `DataGrid` as canonical and support a
    `GridControl` alias only if one already exists in the IDE.
  - Verify: `cargo test -p cobolt-forms data_binding_targets --features render`
    passes. Tests include valid chart/dropdown/list/grid/array cases and invalid
    standalone scalar cases. **Covers AC5, AC5a, AC5b, AC6, AC6a.**

- [x] **T3 — `.cfrm` binding round-trip** (R19, R20, R20a,
  R21a)
  - Files: `crates/cobolt-forms/src/model.rs`,
    existing `.cfrm` load/save module
  - Do: Persist an optional top-level `DataBindings` section. Missing metadata
    deserializes as an empty binding list. Existing control properties,
    including legacy scalar `DataItem` and `DataFormat`, round-trip without
    being promoted into new bindings or changing unrelated XML.
  - Verify: `cargo test -p cobolt-forms data_binding_cfrm --features render`
    passes. Tests assert stable binding metadata, preserved designed casing, and
    unchanged unrelated control properties/events/layout. **Covers AC12, AC16.**

## Layer 2: Guardian validation core

- [x] **T4 — Data Binding Guardian validation module** (R7, R13,
  R14, R15, R15a, R16, R19, R21a, R25, R26)
  - Files: `crates/cobolt-ide/src/data_binding_guardian.rs`,
    `crates/cobolt-ide/src/lib.rs` or `crates/cobolt-ide/src/app.rs`
  - Do: Implement deterministic validation for missing sources, missing target
    controls, missing fields, unsupported targets, incompatible target
    cardinality, missing writable row identity, stale mappings, lossy type
    conversions, and case-insensitive collisions. Return stable `Blocker`,
    `Warning`, and `Info` findings sorted deterministically.
  - Verify: `cargo test -p cobolt-ide data_binding_guardian_core` passes. Tests
    assert blockers/warnings/infos and deterministic ordering. **Covers AC7,
    AC8, AC9, AC19.**

- [x] **T5 — REST and Agent safety rules** (R18, R18a, R22)
  - Files: `crates/cobolt-ide/src/data_binding_guardian.rs`
  - Do: Validate REST and Agent AI bindings only from saved schema, sample
    payload, response data item, or explicit field mappings. Default REST and
    Agent AI bindings to read-only. Block writable REST/Agent bindings without
    update schema, key/row identity mapping, and approved target scope. Block
    Agent AI writes outside the binding target list.
  - Verify: `cargo test -p cobolt-ide data_binding_guardian_rest_agent` passes
    without network access. **Covers AC4, AC5, AC18.**

- [x] **T6 — Binding repair actions** (R23, R24, R24a)
  - Files: `crates/cobolt-ide/src/data_binding_guardian.rs`,
    `crates/cobolt-forms/src/model.rs`
  - Do: Add repair operations for remapping a missing field, removing a mapping,
    marking a binding read-only, refreshing from saved schema/sample metadata,
    refreshing from an available project source, and reselecting a target
    control. Ensure repairs update only binding metadata and do not delete event
    handlers or visual layout.
  - Verify: `cargo test -p cobolt-ide data_binding_repair` passes. Tests assert
    each repair action preserves unrelated controls, properties, events, and
    layout. **Covers AC20.**

## Layer 3: IDE binding editor and action gates

- [x] **T7 — Hide scalar data-binding UI and expose approved targets**
  (R5, R5a, R5b, R5c, R6, R6a, R6b, R8)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/panels/data_binding.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: Remove the universal scalar data-binding section from the Properties
    panel. Show binding affordances only for charts, `ComboBox`, `ListBox`,
    `DataGrid`, and explicit control arrays. For scalar controls inside an
    explicit array, show only array-owned mapping information and no standalone
    source selection.
  - Verify: `cargo test -p cobolt-ide data_binding_properties` and
    `cargo build -p cobolt-ide` pass. Manual check: standalone scalar controls
    do not show binding information; array-member scalar controls show only
    mapping context. **Covers AC6, AC6a.**

- [x] **T8 — Source-specific binding editor** (R2, R3, R4, R17)
  - Files: `crates/cobolt-ide/src/panels/data_binding.rs`,
    `crates/cobolt-ide/src/app.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: Add the binding editor workflow for Indexed `.cidx` definitions, SQL
    result sets, COBOL tables, REST response schemas/samples, and Agent AI
    structured outputs. Support target-specific mapping surfaces for grids,
    charts, dropdowns/lists, and control arrays.
  - Verify: `cargo test -p cobolt-ide data_binding_editor` and
    `cargo build -p cobolt-ide` pass. Manual check: a developer can create
    bindings for each source/target family listed in the spec. **Covers AC1,
    AC2, AC3, AC4, AC5, AC5a, AC5b.**

- [x] **T9 — Guardian gates on save/run/debug/check/build/package**
  (R13, R14, R16, R21)
  - Files: `crates/cobolt-ide/src/app.rs`,
    form/project action handlers in `crates/cobolt-ide/src/**`
  - Do: Route form save, Run Form, Debug, Check, Build, and Package through the
    Guardian before proceeding. Block on `Blocker`, allow with visible findings
    on `Warning`/`Info`, and keep generated-code regeneration contracts intact.
  - Verify: `cargo test -p cobolt-ide data_binding_action_gates` and
    `cargo build -p cobolt-ide` pass. Manual check: blocker findings stop all
    gated actions; warnings do not. **Covers AC7, AC8, AC9, AC13.**

- [x] **T10 — Binding i18n coverage** (R16, R17, R24a)
  - Files: `crates/cobolt-ide/src/i18n.rs`,
    binding editor and Guardian UI call sites
  - Do: Add every binding editor label, source-type label, target label,
    Guardian severity, validation message, warning, and repair action as `Tr`
    fields translated in EN/ES/PT/JA/ZH/FR. Remove hard-coded binding UI text.
  - Verify: `cargo test -p cobolt-ide i18n` passes with no empty binding
    translations. **Covers AC14.**

## Layer 4: Code generation and runtime behavior

- [x] **T11 — Deterministic generated binding code** (R9, R10,
  R10a, R10b, R10c, R11, R12, R21, R21a)
  - Files: `crates/cobolt-codegen/src/lib.rs`,
    `crates/cobolt-codegen/src/data_binding.rs`,
    codegen tests
  - Do: Emit deterministic generated COBOL sections for binding initialization,
    source load, target population, pending edits, and explicit updates. Preserve
    the generated banner and regenerate-on-action behavior. Do not emit syntax
    unsupported by parser, semantic analysis, or runtime.
  - Verify: `cargo test -p cobolt-codegen data_binding_codegen` and
    `cargo test -p cobolt-parser data_binding_generated` pass. Tests parse
    generated Indexed, SQL, COBOL table, REST, Agent, chart, dropdown/list, and
    array examples. **Covers AC10, AC13, AC17.**

- [x] **T12 — Runtime binding state and safe writeback** (R9, R10,
  R10a, R10b, R10c, R11, R12, R18a)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    existing runtime form/source adapter modules
  - Do: Support generated helper calls for initial load, mapped control
    population, pending edit tracking, read-only behavior, row identity
    preservation, explicit update, and failed-update recovery. Reuse existing
    Indexed, SQL, REST, and AgentObject built-ins where possible.
  - Verify: `cargo test -p cobolt-runtime data_binding_runtime` passes. Tests
    assert read-only bindings never write back, initial load does not mark dirty,
    writable updates preserve unmapped fields, and failed updates keep pending
    edits recoverable. **Covers AC10, AC11, AC13, AC18.**

- [x] **T13 — End-to-end generated form checks** (R20, R21,
  R21a)
  - Files: cross-crate integration tests under the existing test structure
  - Do: Add end-to-end fixture forms for each implemented source type and target
    family. Generate COBOL, parse it, run semantic analysis, execute through the
    live interpreter where local fixtures exist, and verify debugger/source-map
    compatibility for generated binding sections and user event handlers.
  - Verify: `cargo test -p cobolt-codegen data_binding_e2e`,
    `cargo test -p cobolt-semantic data_binding_generated`, and
    `cargo test -p cobolt-runtime data_binding_e2e` pass. Manual debugger check:
    breakpoints and stepping still land on expected generated/user lines.
    **Covers AC12, AC13, AC16, AC17.**

## Layer 5: Documentation and finalization

- [x] **T14 — English developer guide update** (R5, R13, R16,
  R17, R24a)
  - Files: `docs/developers-guide-en.md`
  - Do: Document the data-binding workflow, approved target list, scalar-control
    visibility rules, source-specific wiring, Guardian severity levels, REST and
    Agent offline validation, writable identity requirements, and repair
    actions. Do not edit translated guides.
  - Verify: Read the rendered guide section in the IDE documentation viewer or
    Markdown preview. Confirm translated guides are untouched. **Covers AC15.**

- [x] **T15 — Version, changelog, and full verification**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: Apply the required z-version bump per `CONVENTIONS.md`, add a
    top-of-file `CHANGELOG.md` entry dated 2026-06-29, run formatting, run the
    touched-crate tests, then run full verification. Do not commit or push.
  - Verify: `cargo fmt`, `cargo test -p cobolt-forms --features render`,
    `cargo test -p cobolt-ide`, `cargo test -p cobolt-codegen`,
    `cargo test -p cobolt-runtime`, `cargo test -p cobolt-parser`,
    `cargo test -p cobolt-semantic`, `cargo build -p cobolt-ide`, and final
    `cargo test --workspace` all pass or any failures are reported with exact
    blockers. Manual launch check: `cargo run -p cobolt-ide` opens the IDE and
    the binding editor/Guardian gates behave as described above. **Covers all
    ACs.**

---

## AC ↔ Task mapping

| Acceptance criterion | Task coverage |
| --- | --- |
| AC1 — Indexed file to `DataGrid` | T1, T8, T11, T13 |
| AC2 — SQL result set to control array | T1, T8, T11, T13 |
| AC3 — COBOL table to controls in explicit array | T1, T8, T11, T13 |
| AC4 — REST response to `DataGrid` without network validation | T1, T5, T8, T13 |
| AC5 — Agent AI structured response to approved targets | T1, T5, T8, T13 |
| AC5a — Source to chart category/value series | T1, T2, T8, T11 |
| AC5b — Source to `ComboBox`/`ListBox` display and value | T1, T2, T8, T11 |
| AC6 — Disallowed controls hide binding information | T2, T7 |
| AC6a — Scalar array members expose only array mapping info | T2, T7 |
| AC7 — Missing writable row identity is blocked | T4, T9, T12 |
| AC8 — Deleted source/target/field is blocked | T4, T9 |
| AC9 — Lossy conversions warn but can continue | T4, T9 |
| AC10 — Run populates targets without altering unmapped state | T11, T12, T13 |
| AC11 — Writable edits update only mapped fields | T12, T13 |
| AC12 — Existing unbound forms remain unchanged | T3, T13 |
| AC13 — Generated COBOL parses, checks, runs, debugs | T9, T11, T12, T13 |
| AC14 — Binding UI text localized through `Tr` | T10 |
| AC15 — English guide documents workflow/severities | T14 |
| AC16 — `.cfrm` binding round-trip is stable | T3, T13 |
| AC17 — Debugging preserves breakpoint/step/source mapping | T11, T13 |
| AC18 — REST/Agent validate offline | T5, T12 |
| AC19 — Ambiguous case-insensitive references blocked | T4 |
| AC20 — Repair actions preserve events/layout | T6 |

## Done criteria

All acceptance criteria in `spec.md` are satisfied, every task verification has
been run and reported with real results, docs and i18n are updated, the required
version/changelog changes are present, and no commit or push is made unless the
operator explicitly asks.
