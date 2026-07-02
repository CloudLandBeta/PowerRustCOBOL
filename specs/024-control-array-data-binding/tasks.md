# Tasks — Data Binding for Control Arrays (repeating GroupBox)

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-07-01

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. The project should build &
test green after each task. Do **not** commit or push.

---

- [ ] **T1 — Indexed member-chain helper** (R2)
  - Files: `crates/cobolt-forms/src/model.rs` (or a small helper module).
  - Do: Add a helper to build and parse the spec-011 chain
    `ArrayName(i)::ChildId::Property` (1-based `i`). Single source of truth used by
    the renderer, runtime population, and (future) event dispatch.
  - Verify: `cargo test -p cobolt-forms` — round-trip unit test (build → parse →
    equal) for several ids/properties/indices, including case-insensitive array id.

- [ ] **T2 — Runtime instance rendering of a repeating GroupBox** (R13, spec-015
  Phase 3 foundation)
  - Files: `crates/cobolt-forms/src/render.rs`, `crates/cobolt-forms/src/model.rs`
    (instance-count accessor).
  - Do: In the shared renderer, when a top-level GroupBox has
    `IsRepeatingGroup = true`, expand it into `ItemCount` instances laid out by its
    Vertical/Horizontal/Grid layout inside its auto-scroll Panel (mirror the
    Designer-preview clone logic). Non-repeating GroupBoxes are byte-for-byte
    unchanged. Members keep template values for now.
  - Verify: `cargo test -p cobolt-forms --features render` — a plain GroupBox
    renders 1 (unchanged); a repeating GroupBox with `ItemCount = k` produces k
    instances at expected offsets. `cargo build -p cobolt-ide` green.

- [ ] **T3 — Per-instance member value resolution in the renderer** (R14 display,
  R11)
  - Files: `crates/cobolt-forms/src/render.rs`.
  - Do: Each rendered instance `i` resolves each member's displayed property from
    the object model at `ArrayName(i)::Child::Property` (via T1), falling back to
    the member's design-time value when unset. Unmapped members show their design
    value.
  - Verify: `cargo test -p cobolt-forms --features render` — with seeded
    per-instance values, instance `i` shows its own value; a member with no seeded
    value shows the template value.

- [ ] **T4 — Member default bindable-property table** (R10)
  - Files: `crates/cobolt-ide/src/panels/data_binding.rs`.
  - Do: Promote `default_member_property` to the full per-control-type table
    (Label/Button→Caption, TextBox→Text, CheckBox/RadioButton→Checked,
    ComboBox/ListBox→Value, PictureBox→ImagePath,
    NumericUpDown/Slider/ProgressBar/DateTimePicker→Value); expose the list of
    overridable **writable** properties per type. **Resolve spec Q7 here.**
  - Verify: `cargo test -p cobolt-ide` — default property is correct for each
    control type; overridable set matches the table.

- [ ] **T5 — ControlArray mapping editor (member + property pickers)** (R9, R11,
  R12; sources R7/R8)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/panels/data_binding.rs`,
    `crates/cobolt-ide/src/i18n.rs`.
  - Do: When the approved target is a `ControlArray`, render one mapping row per
    source field: a member-control picker (restricted to the group's members) and
    a property picker (default from T4, overridable). Reject a second field
    targeting the same `(control_id, property_name)`. Confirm all five source
    editors are available unchanged. New UI strings via `Tr` (all six languages).
  - Verify: `cargo test -p cobolt-ide` — `default_mappings_for_target` yields one
    `ControlProperty` per member with the right default; duplicate
    `(control, property)` is rejected; `cargo build -p cobolt-ide` green. Covers
    AC3, AC4, AC5.

- [ ] **T6 — Target eligibility & array-member info panel** (R1, R5, R6)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/panels/data_binding.rs`.
  - Do: A repeating GroupBox is offered as a **Control Array** target; a plain
    GroupBox and non-GroupBox controls are not. A selected **member** control shows
    only its array-member mapping info (`ArrayMemberMapping`), never a scalar
    binding.
  - Verify: `cargo test -p cobolt-ide` — `visibility_for_control` returns
    `ApprovedTarget(ControlArray)` only for a repeating GroupBox, `ArrayMemberMapping`
    for its children, `Hidden` otherwise. Covers AC1, AC2.

- [ ] **T7 — Design-time apply / preview seeding** (R15, R16)
  - Files: `crates/cobolt-ide/src/app.rs`.
  - Do: Add a `ControlArray` branch to `apply_data_binding_target_properties`: set
    the group's instance count from the (preview) row count and seed preview member
    values from `data_binding_preview_rows`, **preserving** unmapped members'
    design values and any user overrides (preserve-if-set, like the DataGrid
    cobol_mask fix).
  - Verify: `cargo test -p cobolt-ide` — after apply, instance count matches preview
    rows and mapped members carry preview values while unmapped/overridden values
    are preserved; re-apply is idempotent. Covers AC8; supports AC7.

- [ ] **T8 — Runtime population: `refresh_control_array_binding`** (R14)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`.
  - Do: Analogous to `refresh_datagrid_binding`: derive row count from the source
    (COBOL-table `FIELD(n)` dims first), set the group's `ItemCount`, and for each
    instance `i` set each mapped member property `ArrayName(i)::Member::Property`
    from row `i`, formatted through the field COBOL mask (reuse spec-023 formatter).
    Clear stale instances when the data set shrinks.
  - Verify: `cargo test -p cobolt-runtime` — populated per-instance values match
    the `FIELD(n)` values with mask formatting; shrinking the data set leaves no
    stale instance. Covers AC6, AC7.

- [ ] **T9 — Writable member writeback** (R17, R18, R19)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`.
  - Do: On a writable binding, pushing a changed **editable** member value updates
    only the mapped source field for that instance's row (via
    `binding_set_pending`/`binding_update`, key = key-field value). Read-only never
    writes. A failed update keeps the pending value.
  - Verify: `cargo test -p cobolt-runtime` — writable edit updates the right
    row/field only; read-only edit does not write; forced-failure path preserves the
    pending value. Covers AC9.

- [ ] **T10 — Seeding & deterministic codegen** (R21)
  - Files: `crates/cobolt-ide/src/form_runtime.rs`,
    `crates/cobolt-codegen/src/data_binding.rs`.
  - Do: Extend `append_data_binding_seed_props` to seed ControlArray binding
    metadata (kind, field→member.property map, key fields). Add a
    `write_control_array_seed` analog to `write_datagrid_refresh_seed`; keep
    LOAD/POPULATE/MARK-CLEAN/UPDATE emission deterministic.
  - Verify: `cargo test -p cobolt-codegen` — generated array-binding sections are
    byte-stable across runs, carry the banner, and list `field -> ControlProperty`
    comments; `cargo test -p cobolt-ide` seed test. Supports AC11.

- [ ] **T11 — Guardian: display-only-member rule + gate coverage** (R20)
  - Files: `crates/cobolt-ide/src/data_binding_guardian.rs`,
    `crates/cobolt-ide/src/i18n.rs`.
  - Do: A **writable** mapping whose member property is on a display-only control
    (Label, PictureBox, ProgressBar) is treated as read-only-by-nature (Warning, no
    writeback) — **resolve spec Q6 here**. Confirm ControlArray bindings are
    validated and blocked on save/run/debug/check/build/package for unsafe mappings
    (missing member, missing writable key, ambiguous ref, disallowed conversion).
  - Verify: `cargo test -p cobolt-ide` — display-only writable member yields a
    Warning not a Blocker; each action gate blocks a deliberately-unsafe array
    binding and passes a safe one. Covers AC10.

- [ ] **T12 — `.cfrm` round-trip & generated-form checks** (R22, R23)
  - Files: tests in `crates/cobolt-forms` and `crates/cobolt-codegen` (+ parser as
    needed).
  - Do: Add end-to-end checks: an array-bound form saves→loads identical; a repair
    action that remaps/removes a member preserves group layout and member event
    handlers; the generated COBOL parses and checks.
  - Verify: `cargo test -p cobolt-forms`, `cargo test -p cobolt-codegen`,
    `cargo test -p cobolt-parser` — round-trip equal; repair preserves layout/events;
    generated program parses/checks. Covers AC12, AC13, AC11.

- [ ] **T13 — Docs & i18n** (R24, R25)
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`.
  - Do: Document the array-binding workflow and the DataGrid-vs-array differences
    table in the English guide (translations untouched). Ensure every new `Tr` key
    is filled in all six languages.
  - Verify: `cargo test -p cobolt-ide` i18n completeness test (no empty
    translations); guide section renders. Covers AC14, AC15.

- [ ] **T14 — Finalize: version, changelog, full verification**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`.
  - Do: Apply the version bump + top `CHANGELOG.md` entry. **Resolve spec Q5:**
    functionally a feature (minor per tech.md), but the operator's pre-production
    standing rule treats all changes as z-level fixes — default to a **z-bump /
    fix** entry unless the operator says otherwise. Run `cargo fmt`, the touched
    crate tests, and a full build.
  - Verify: `cargo fmt --check`; `cargo test -p cobolt-forms --features render`,
    `-p cobolt-ide`, `-p cobolt-runtime`, `-p cobolt-codegen`, `-p cobolt-parser`;
    `cargo build -p cobolt-ide` — all green or failures reported exactly. Manual:
    `cargo run -p cobolt-ide` — design a repeating "customer card" GroupBox, bind to
    a COBOL table, confirm N cards render populated across Designer/Preview/Run, and
    a writable TextBox edit writes back.

---

## AC ↔ Task mapping

| Acceptance criterion | Task(s) |
| --- | --- |
| AC1 — repeating GroupBox offered; others not | T6 |
| AC2 — member shows only array-member mapping | T6 |
| AC3 — all five sources selectable | T5 |
| AC4 — field → member + property, default overridable | T4, T5 |
| AC5 — no two fields to same member property | T5 |
| AC6 — K rows → K populated, masked instances | T2, T3, T8 |
| AC7 — unmapped keep design values; no stale | T3, T7, T8 |
| AC8 — preview from definitions | T7 |
| AC9 — writeback correct / read-only / failure preserved | T9 |
| AC10 — gates block unsafe mappings | T11 |
| AC11 — generated COBOL parses/checks/runs/debugs | T10, T12 |
| AC12 — `.cfrm` round-trip; others unaffected | T12 |
| AC13 — repair preserves layout/events | T12 |
| AC14 — i18n six languages | T13 |
| AC15 — English guide documents workflow/differences | T13 |

## Done criteria
All acceptance criteria in `spec.md` are satisfied, every task's verification has
been run and reported with real results, docs and i18n are updated, the version/
CHANGELOG change is applied per the resolved Q5 classification, and no commit or
push is made unless the operator explicitly asks.
