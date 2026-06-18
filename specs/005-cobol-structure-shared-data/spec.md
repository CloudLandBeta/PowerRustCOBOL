# Spec — Form COBOL Structure, shared data (GLOBAL/EXTERNAL), and Rust FFI

- **Status:** draft → approved
- **Folder:** specs/005-cobol-structure-shared-data/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-18

## 1. Overview

RAD forms today declare controls, events, and a per-event handler body, but have
**no place for shared COBOL declarations** — special-names, file definitions,
working-storage — that the generated outer program and its nested event/user
procedures (and other forms, and CALLed common code) can see. This feature adds
an editable **"COBOL Structure"** surface to each form, mirroring the Fujitsu
**PowerCOBOL form-module** layout, woven by codegen into the form's outer
program. It formalises **GLOBAL** (intra-form) and **EXTERNAL** (run-unit-wide,
cross-form + common-code) data sharing — with correct per-form namespacing and a
real run-unit-wide EXTERNAL store — and adds the **COBOL-2002** sections
(`REPOSITORY`, `BASED-STORAGE`, `CONSTANT`) the model needs. Building on
`REPOSITORY`, it introduces a **Rust-FFI bridge**: REPOSITORY maps COBOL names to
Rust types, `USAGE OBJECT REFERENCE` items hold Rust object handles, and
`INVOKE`/`obj::method()` calls Rust library functions.

## 2. Goals / Non-goals

- **Goals:**
  - A per-form **COBOL Structure editor** with fixed division/section scaffolding
    and editable blocks for `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`,
    `FILE SECTION`, `WORKING-STORAGE`.
  - **User procedures**: developer-written named COBOL procedures, woven as
    nested programs in the form, that see the form's GLOBAL data and are callable
    from event handlers and from each other.
  - **Correct GLOBAL / EXTERNAL / GLOBAL EXTERNAL** sharing: form-private globals
    namespaced per form; externals shared run-unit-wide by real name.
  - **COBOL-2002 language support** for `REPOSITORY`, `BASED-STORAGE`,
    `CONSTANT`, and `USAGE OBJECT REFERENCE` (lexer/parser/AST/semantic/runtime).
  - A **run-unit-wide EXTERNAL store** shared across all running forms + CALLed
    common code.
  - A **Rust-FFI bridge** via REPOSITORY: Rust-type bindings, object-reference
    handles, `INVOKE`/`::` calls into Rust, a realistic first-cut type set with a
    defined handle/drop lifecycle.
- **Non-goals:**
  - Full COBOL-2002 OO authored *in COBOL* (classes, inheritance, methods) —
    only the `OBJECT REFERENCE` + `REPOSITORY` needed for the bridge and shared
    declarations.
  - Automatic binding generation for arbitrary Rust crates — a **curated/
    registered** bridge surface in the first cut.
  - Editing `LOCAL-STORAGE`/`LINKAGE`/`SCREEN`/`REPORT`/`COMMUNICATION` in this
    surface.
  - Hand-editing generated `.cbl`.

## 3. User stories

- As a developer, I want a place on the form to declare shared working-storage
  and file definitions, so my event handlers (and other forms) can use them.
- As a developer, I want to mark data **GLOBAL/EXTERNAL** so I control whether it
  is form-private or shared across the whole project at run time.
- As a developer, I want to call a **Rust library** from COBOL by declaring a
  Rust object reference and `INVOKE`-ing methods on it.

## 4. Requirements (EARS)

**COBOL Structure editor**
- **R1 (ubiquitous):** The IDE shall provide, per form, an editable "COBOL
  Structure" surface with **fixed** division/section scaffolding and **editable**
  code blocks for `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`,
  `FILE SECTION`, `WORKING-STORAGE`.
- **R2 (ubiquitous):** The form model (`.cfrm`) shall persist each editable
  block's COBOL source.
- **R3 (event):** When the form is Built/Run/Debugged/Checked, the system shall
  weave the blocks into the generated outer program in correct COBOL order
  (ENVIRONMENT→CONFIGURATION→`SPECIAL-NAMES`/`REPOSITORY`;
  ENVIRONMENT→INPUT-OUTPUT→`FILE-CONTROL`; DATA→`FILE`/`WORKING-STORAGE`),
  preserving the developer banner and the regenerate-on-action contract.

**User procedures**
- **R3a (ubiquitous):** The IDE shall let the developer define named **user
  procedures** (raw COBOL procedure bodies) on a form.
- **R3b (event):** When the form is generated, each user procedure shall be woven
  as a nested program in the form's outer program that can see the form's GLOBAL
  data and is callable (by name) from event handlers and from other user
  procedures.

**COBOL-2002 language support**
- **R4:** The lexer/parser/AST/semantic shall recognise the `REPOSITORY`
  paragraph and the `USAGE OBJECT REFERENCE <name>` clause. (BASED-STORAGE and
  CONSTANT are out of scope.)
- **R5 (constraint):** Invalid/unsupported content in an editable block shall
  surface as a build/check **diagnostic**, never a silent failure.

**Data sharing**
- **R6:** A `01`/`77` item declared `GLOBAL` in a form's COBOL Structure shall be
  visible to that form's nested event/user procedures and **not** to other forms
  (unless also `EXTERNAL`).
- **R7:** A `01`/`77` item or `FD` declared `EXTERNAL` shall be shared
  **run-unit-wide by its real name** across all forms and CALLed common code; the
  system shall back this with a run-unit-wide store.
- **R8:** A `GLOBAL EXTERNAL` item shall be **both** run-unit-shared (by real
  name) **and** visible to the declaring form's nested procedures.
- **R9 (constraint):** Non-`EXTERNAL` `GLOBAL` items shall be namespaced per form
  (`form-name.item`) internally; `EXTERNAL`/`GLOBAL EXTERNAL` items shall keep
  their **real** names.
- **R10 (constraint):** The system shall reject (diagnostic) an `EXTERNAL` clause
  on items that are not `01`/`77`/`FD`, and should warn when two `EXTERNAL`
  declarations of the same name have differing descriptions.

**Sharing UI / i18n**
- **R11:** The `WORKING-STORAGE`/`FILE` editing experience shall let the developer
  designate items as `GLOBAL`, `EXTERNAL`, or `GLOBAL EXTERNAL` (exact UX — see
  Open Question Q1).
- **R12 (constraint):** All new IDE UI strings shall be translated in **all six**
  languages (`i18n.rs`).

**Rust FFI (may be a later phase — see Q2)**
- **R13:** A `REPOSITORY` entry shall bind a COBOL name to a Rust type, e.g.
  `RUST-STRING` ↦ `"Rust.String"`.
- **R14:** A data item `nn NAME USAGE OBJECT REFERENCE <repo-name>` shall declare
  a **handle** to a Rust object of the bound type.
- **R15 (event):** When COBOL executes `INVOKE NAME "method" [USING …]
  [RETURNING …]` or the inline `NAME::method(…)`, the runtime shall invoke the
  corresponding Rust function on the referenced object, **marshaling**
  arguments/results between COBOL and Rust.
- **R16:** The first cut shall support a realistic Rust type set (at least
  `i32`/`i64`/`f64`/`bool`, `String`, `Vec`) with object-handle creation and a
  drop lifecycle; the set is expandable.
- **R17 (constraint):** Rust objects referenced from COBOL shall have a defined
  ownership/drop lifecycle so they are released (no leaks) — model TBD (Q3).

## 5. Acceptance criteria

- [ ] AC1 — A form can define non-empty `SPECIAL-NAMES`/`REPOSITORY`/
  `FILE-CONTROL`/`FILE`/`WORKING-STORAGE` blocks; they persist in `.cfrm` and
  appear, in correct order, in the generated `.cbl` under the banner.
- [ ] AC2 — A `GLOBAL` item in a form's `WORKING-STORAGE` is readable/writable
  from that form's event handler at run time, and is **not** visible to a second
  form unless `EXTERNAL`.
- [ ] AC3 — An `EXTERNAL` item set by form A is observed with the same value by
  form B and by CALLed common code at run time (shared store).
- [ ] AC4 — A `GLOBAL EXTERNAL` item is both shared cross-form and usable in the
  declaring form's handlers without re-declaration.
- [ ] AC5 — `REPOSITORY` and `USAGE OBJECT REFERENCE` parse and build with no
  error; invalid content yields a clear diagnostic.
- [ ] AC8 — A user procedure defined on a form is callable by name from an event
  handler at run time, and can read/write the form's GLOBAL data.
- [ ] AC6 — Demo: `REPOSITORY` binds `RUST-STRING` ↦ `Rust.String`;
  `05 S USAGE OBJECT REFERENCE RUST-STRING`; `INVOKE S "len"` (or `S::len()`)
  returns the Rust string length; the object is dropped with no leak.
- [ ] AC7 — New IDE strings exist in all six languages; English dev guide updated;
  generated banner preserved; no hand-edited generated `.cbl`.

## 6. Constraints & steering check

- **i18n (6 languages):** New COBOL-Structure editor strings → `Tr` fields in all
  six languages.
- **Generated-code / regenerate contract:** Blocks are woven into the generated
  outer program, regenerated on Build/Run/Debug/Check, banner preserved;
  generated `.cbl` stays a build artifact.
- **Docs (English guide):** New section covering the COBOL Structure surface, the
  GLOBAL/EXTERNAL model, and the Rust FFI. Translations untouched.
- **Fix vs feature:** **Feature** → minor (`y`) bump + `CHANGELOG.md`.
- **COBOL identifiers/source English; no "cobolt" in UI.**

## 7. Open questions

- **Q1 (sharing UX):** GLOBAL/EXTERNAL designation — three labelled sub-areas
  where the IDE injects the clauses, vs the developer writing `GLOBAL`/`EXTERNAL`
  clauses inline. *Recommendation:* developer writes the clauses (full COBOL
  control), editor offers guidance/validation; revisit after first use.
- **Q2 (FFI phasing):** ship COBOL Structure + sharing first and the **Rust FFI
  as a second phase / its own spec**? *Recommendation:* phase the FFI after
  structure+sharing lands (it is the riskiest, deepest part).
- **Q3 (FFI types/ownership):** the exact first-cut Rust type set and the
  ownership/drop model (RAII handle table; explicit `Drop`/`Free` method vs
  scope-based release). Resolve in `/plan`.
- **Q4 (per-form user procedures): RESOLVED — included.** This feature adds a
  per-form user-procedure editable area (R3a/R3b).
- **Q5 (BASED-STORAGE/CONSTANT): RESOLVED — dropped.** Both sections are out of
  scope.
