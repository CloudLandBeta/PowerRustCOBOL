# Plan — Data Binding for Control Arrays (repeating GroupBox)

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-01

## 1. Approach

Bind a repeating GroupBox to a data source by **reusing the entire spec-022/023
binding pipeline** and adding only the control-array *target* behaviour on top. A
binding whose target is `ControlArray { array_id, member_control_ids }` maps each
source field to a `ControlProperty { array_id, control_id, property_name }`
(both types already exist). The work is layered:

- **A. Foundation (prerequisite): runtime instance rendering.** Today only the
  Designer *previews* repeating-group clones (`designer.rs` `PreviewItemCount`);
  the running form does not expand a repeating GroupBox into N instances, and
  there is no per-instance member value store. Data binding (R13/R14) is
  meaningless without this. So the first phase delivers spec-015 Phase 3:
  the running renderer expands a repeating GroupBox into `ItemCount` instances
  laid out by its layout inside its auto-scroll Panel, and each instance's member
  values resolve through the object model at `ArrayName(i)::ChildId::Property`
  (spec 011). This is the highest-risk, highest-value piece (§5).

- **B. Target UI (R9–R12).** In the existing binding editor
  (`properties.rs` + `panels/data_binding.rs`), when the approved target is a
  `ControlArray`, render one mapping row per source field: **member-control
  picker** (restricted to the group's members) + **property picker** (default
  per control type, overridable to a writable property). Enforce "no two fields
  to the same `(control, property)`" (R12). A member control selected in the
  canvas keeps showing only its array-member info (`ArrayMemberMapping`), never a
  scalar binding (R6). `default_member_property` is promoted to a full
  per-control-type table (R10).

- **C. Design-time apply / preview (R15/R16).** Add a `ControlArray` branch to
  `apply_data_binding_target_properties` (app.rs): set the group's instance count
  from the (preview) row count and seed preview member values, **preserving**
  unmapped members' design-time values and any user overrides (same
  preserve-if-set discipline as the DataGrid cobol_mask fix). Reuse
  `data_binding_preview_rows`.

- **D. Runtime population & writeback (R14/R17–R19).** Add
  `refresh_control_array_binding(array_id)` to the interpreter, analogous to
  `refresh_datagrid_binding`: derive row count from the source (for COBOL tables,
  from the mapped `FIELD(n)` array dims exactly like the grid path), set the
  group's `ItemCount`, and for each instance `i` set each mapped member property
  `ArrayName(i)::Member::Property` from row `i`, formatted through the field's
  COBOL mask (reuse the spec-023 mask formatter). Writable bindings push a
  changed **editable** member value back via the existing `binding_set_pending`
  / `binding_update` state and a control-array update path (R17); read-only never
  writes (R18); failed updates keep the pending value (R19).

- **E. Seeding & codegen (R21).** Extend `append_data_binding_seed_props`
  (form_runtime) and add a `write_control_array_seed` analog to
  `write_datagrid_refresh_seed` (codegen) so the interpreter receives the array's
  binding kind, the field→member.property map, and key fields. The generic
  `COBOL-DATA-BINDINGS-LOAD/POPULATE/MARK-CLEAN/UPDATE` paragraphs already cover
  arrays; `target_label`/`target_path_label` already print `ControlArray`/
  `ControlProperty`.

- **F. Guardian (R20).** ControlArray target resolution, `ControlProperty`
  mapping, source-field collisions, and writable-key checks already exist. Add
  one rule: a **writable** mapping whose member property is on a display-only
  control type (Label, PictureBox, ProgressBar) is treated as read-only-by-nature
  — no writeback attempted — surfaced as a Warning, not a Blocker (spec Q6).

- **G. Docs & i18n (R24/R25).** New `Tr` keys ×6; English guide section with the
  DataGrid-vs-array differences table.

## 2. Affected crates / files

- `crates/cobolt-forms/src/render.rs` — **A**: expand a repeating GroupBox into
  `ItemCount` runtime instances; resolve member values from the object model per
  instance. (Shared renderer, so Preview/Run/binary all benefit.)
- `crates/cobolt-forms/src/model.rs` — helpers: promote member default-property
  table; any `ItemCount`/instance accessors needed by the renderer. Target/path
  types already present.
- `crates/cobolt-ide/src/form_runtime.rs` — **E**: seed ControlArray binding
  metadata into the interpreter object model before run.
- `crates/cobolt-runtime/src/interpreter.rs` — **D**: `refresh_control_array_
  binding`, per-instance member `SetProperty`, control-array writeback path.
- `crates/cobolt-ide/src/panels/data_binding.rs` — **B**: member/property pickers,
  full `default_member_property` table, mapping helpers for ControlArray.
- `crates/cobolt-ide/src/panels/properties.rs` — **B**: render the ControlArray
  mapping editor (member + property per field); array-member info panel.
- `crates/cobolt-ide/src/app.rs` — **C**: `apply_data_binding_target_properties`
  ControlArray branch (instance count + preview seed, preserve unmapped).
- `crates/cobolt-ide/src/data_binding_guardian.rs` — **F**: writable-vs-display-
  only member property rule.
- `crates/cobolt-codegen/src/data_binding.rs` — **E**: `write_control_array_seed`
  analog; deterministic ordering.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` keys in all six languages.
- `docs/developers-guide-en.md` — array-binding workflow + differences (English
  only; translations untouched).
- `CHANGELOG.md` + `crates/cobolt-ide/src/version.rs` — version bump (see §7).

## 3. Data / model changes

- **No new binding types** — `ApprovedBindingTargetKind::ControlArray`,
  `BindingTargetDescriptor::ControlArray`, `BindingTargetPath::ControlProperty`
  already exist and serialize in `.cfrm` (R22, unchanged round-trip).
- **`ItemCount`** (existing GroupBox prop) becomes the runtime instance count the
  renderer honours (currently inert). Design-time preview keeps
  `PreviewItemCount`.
- **Per-instance member values**: stored in the interpreter object model keyed by
  the spec-011 indexed chain (`ArrayName(i)::ChildId::Property`); no `.cfrm`
  schema change (runtime-only state).
- **Seeded metadata** (`_BindingKind`, field→member.property map, key fields):
  transient runtime props on the array control, mirroring the DataGrid
  `_BindingKind`/`_BindingFields` convention. Not persisted.
- Member default-property table (code, not persisted): Label/Button→Caption,
  TextBox→Text, CheckBox/RadioButton→Checked, ComboBox/ListBox→Value,
  PictureBox→ImagePath, NumericUpDown/Slider/ProgressBar/DateTimePicker→Value.

## 4. Key decisions & alternatives

- **Decision:** Deliver spec-015 Phase-3 runtime instance rendering as phase A of
  this feature. — **Why:** binding population has no visible effect without it,
  and it is the shared foundation for indexed addressing/events. — **Rejected:**
  binding against Designer-preview clones only (no real runtime behaviour); or
  declaring it an external blocker and shipping a UI that does nothing at runtime.
- **Decision:** Reuse the DataGrid runtime population pattern
  (`refresh_*_binding` reading `FIELD(n)` dims + object-model `SetProperty`). —
  **Why:** proven, deterministic, already wired to the generated CALLs. —
  **Rejected:** a new bespoke array-binding runtime stack (violates the reuse
  constraint).
- **Decision:** Field→property mapping is 1 field → 1 `(member, property)`, unmapped
  members stay static. — **Why:** matches the existing `ControlProperty` model
  and spec R9/R11; predictable. — **Rejected:** implicit auto-mapping by
  name-matching every member (surprising, collision-prone).
- **Decision:** Writeback only from **editable** member types; display-only
  members are read-only-by-nature even in a writable binding (Q6). — **Why:** you
  cannot "edit" a Label/PictureBox at runtime. — **Rejected:** blocking the whole
  binding when any mapped member is display-only (too strict).
- **Decision:** Preserve unmapped/overridden member values on refresh
  (preserve-if-set). — **Why:** avoids the DataGrid cobol_mask class of bug where
  refresh wiped user intent. — **Rejected:** unconditional reseed.

## 5. Risks & mitigations

- **Risk (highest): runtime instance rendering is new and touches the shared
  renderer** → could regress non-repeating GroupBoxes / spec-012 clipping.
  **Mitigation:** gate all new behaviour on `IsRepeatingGroup`; snapshot/unit
  tests for a plain GroupBox (unchanged) vs a repeating one (N instances);
  reuse the Designer-preview clone logic as the reference for layout.
- **Risk: per-instance member addressing must agree between renderer, object
  model, and event dispatch (spec 011/015).** → **Mitigation:** single helper
  that builds/parses `ArrayName(i)::ChildId::Property`; unit-test round-trip;
  keep event dispatch out of v1 scope if not required for binding display.
- **Risk: source row-count derivation differs per source (COBOL table dims vs
  SQL/REST loaded rows).** → **Mitigation:** v1 runtime population mirrors the
  DataGrid path (COBOL-table `FIELD(n)` dims proven); other sources populate via
  the same loaded-rows abstraction the grid uses for preview; verify per source
  in tests.
- **Risk: writeback identity for arrays (which row a member edit maps to).** →
  **Mitigation:** reuse `binding_set_pending`/`row_key` with the instance index →
  key-field value; guardian blocks writable-without-key (existing R20 rule).
- **Risk: scope creep into full spec-015 event dispatch.** → **Mitigation:**
  this plan covers *display + writeback binding only*; indexed event dispatch
  (spec 015 Phase 4) stays out unless a task explicitly needs it.

## 6. Test strategy

- **cobolt-forms** (`render.rs`, `model.rs`): unit tests that a plain GroupBox is
  unchanged; a repeating GroupBox with `ItemCount = k` yields k instances in the
  configured layout; `ArrayName(i)::Child::Property` build/parse round-trips.
  Report counts/positions, not screenshots.
- **cobolt-runtime** (`interpreter.rs`): `refresh_control_array_binding` sets the
  instance count and each mapped member property from `FIELD(n)` values with mask
  formatting; read-only never writes back; writable pushes only mapped fields and
  preserves the pending value on failure. Assert exact populated values.
- **cobolt-codegen** (`data_binding.rs`): generated LOAD/POPULATE/UPDATE for a
  ControlArray binding is deterministic and stable across runs; carries the
  banner; comments list `field -> ControlProperty`. Parse the output.
- **cobolt-ide** (`data_binding.rs`, `app.rs`, `data_binding_guardian.rs`):
  default mappings for a ControlArray target pick the right member property per
  control type; a member selected shows array-member info only; two fields to the
  same member property are rejected; design-time apply seeds instance count +
  preview values while preserving unmapped members; guardian blocks unsafe array
  mappings on each action gate and treats display-only writable members as
  read-only-by-nature.
- **Manual/visual:** `cargo run -p cobolt-ide`; design a "customer card" GroupBox,
  mark repeating, bind to a COBOL table; confirm N cards render with the right
  fields in Designer preview, Preview, and Run; edit a writable TextBox and
  confirm writeback; verify parity across the three surfaces.

## 7. Steering compliance
- [x] i18n: all new binding-UI strings added as `Tr` fields in 6 languages
  (`i18n.rs`); no literals.
- [x] Generated-code banner + regenerate-on-action preserved: array binding flows
  through `write_header` + `regenerate_all_forms`; new seed is deterministic.
- [x] English dev guide updated (`developers-guide-en.md`); translations untouched.
- [ ] Fix vs feature: functionally a **feature** (minor bump per tech.md), but the
  operator's pre-production standing rule treats changes as **z-level fixes** —
  **confirm at `/tasks`/implement** which applies (spec Q5).
- [x] No "cobolt" in user-facing text; COBOL identifiers/source stay English.

## 8. Gate
Review this `plan.md`. When satisfied, run **`/tasks`** to break it into an
ordered, verifiable task list. Note the phase-A dependency (runtime instance
rendering) and the Q5 version-classification and Q6/Q7 questions carried from the
spec — resolve them at `/tasks`. Do not write tasks or code yet.
