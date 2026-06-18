# Plan — Form COBOL Structure, shared data (GLOBAL/EXTERNAL), and Rust FFI

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-18

## 1. Approach

Extend the existing **free-COBOL-block** pattern. The Form already carries
`user_ws_source` — a raw COBOL block that `cobolt-codegen::generate` weaves into
`WORKING-STORAGE` (lib.rs:302). We generalise that into a **`CobolStructure`**:
one editable raw-COBOL block per fixed section. The developer writes real COBOL
(including `GLOBAL`/`EXTERNAL` clauses — Q1 resolved: dev writes the clauses);
codegen weaves each block into the correct division/section of the form's outer
program; the lexer/parser/semantic/runtime are extended to understand the
COBOL-2002 constructs so the woven program parses, checks, and runs (R1–R5).

**Phasing (Q2 resolved):**
- **Phase 1 — COBOL Structure + user procedures + GLOBAL/EXTERNAL sharing.**
  Editor + model + codegen weaving for `SPECIAL-NAMES`, `FILE-CONTROL`, `FILE`,
  `WORKING-STORAGE`; **user procedures** (woven as nested programs, R3a/R3b);
  semantic for `EXTERNAL`/`GLOBAL`; the run-unit-wide **EXTERNAL store**
  (R6–R10). *(BASED-STORAGE and CONSTANT are dropped — Q5.)*
- **Phase 2 — Rust FFI.** `REPOSITORY` Rust-type bindings, `USAGE OBJECT
  REFERENCE`, `INVOKE`/`::` dispatch into a curated Rust registry, object
  handle + drop lifecycle (R13–R17).

**Data sharing model (how the semantics are realised):**
- **GLOBAL** is already shared from the outer program to nested handlers
  (`environment.rs::global_items_from_data_division`). Per-form **isolation** is
  *automatic*: each form runs its own interpreter state, so `GLOBAL X` in form A
  and form B never collide — no literal `form-name.item` mangling needed at the
  storage layer; the spec's namespacing is satisfied by isolation (R6, R9).
- **EXTERNAL** items route to a **run-unit-wide shared store** keyed by the
  **real** name, shared by every running form interpreter and CALLed common code
  (R7). `GLOBAL EXTERNAL` = routed to the shared store **and** exported to nested
  handlers (R8).

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs` — add `CobolStructure { special_names,
  repository, file_control, file_section, working_storage: String }` and
  `user_procedures: Vec<UserProcedure { name, code }>` to `Form`; fold today's
  `user_ws_source` into `working_storage` (keep a compat alias on load).
- `crates/cobolt-forms/src/xml.rs` — serialise/deserialise the new blocks +
  user procedures (optional elements → empty default; old `.cfrm` still loads).
- `crates/cobolt-codegen/src/lib.rs` — `generate()` weaves each block into its
  division/section in order (ENV→CONFIG→`SPECIAL-NAMES`/`REPOSITORY`;
  ENV→I-O→`FILE-CONTROL`; DATA→`FILE`/`WORKING-STORAGE`), verbatim, under the
  banner; emits each **user procedure** as a nested program (sibling to the
  event-handler nested programs, callable by name); regenerate-on-action
  preserved.
- `crates/cobolt-lexer/src/{keywords.rs,token.rs}` — tokens for `REPOSITORY`,
  `OBJECT` (reuse `REFERENCE`).
- `crates/cobolt-ast/**` — a `Repository` config entry (name ↦ external class
  string); `Usage::ObjectRef`.
- `crates/cobolt-parser/src/{data.rs,parser.rs,...}` — parse `REPOSITORY` in the
  CONFIGURATION SECTION; parse `USAGE OBJECT REFERENCE <name>` on data items
  (GLOBAL/EXTERNAL already parsed).
- `crates/cobolt-semantic/**` — `EXTERNAL` only on `01`/`77`/`FD` (diagnostic
  otherwise, R10); warn on differing descriptions for same EXTERNAL name;
  resolve `REPOSITORY` bindings.
- `crates/cobolt-runtime/src/environment.rs` + `interpreter.rs` — a new
  **`external_store`** (run-unit-wide `Arc<Mutex<HashMap<String, CobolValue>>>`):
  on program init, register `EXTERNAL` items by real name (create-if-absent);
  route their reads/writes to the store. Ensure **user-procedure nested programs
  are callable by name** from event handlers (reuse the existing nested-program
  CALL dispatch used by the event loop). *(Phase 2)* a `RustBridge` registry
  (repo-name ↦ constructor/method table) + an object handle table (id ↦
  `Box<dyn Any>`) with drop.
- `crates/cobolt-ide/src/panels/cobol_structure.rs` *(new)* — the editor: one
  COBOL code-editor block per section, plus a **user-procedures** area (list +
  per-procedure code editor); reuse the existing editor widget; read/write
  `Form.cobol_structure` / `Form.user_procedures`. Wired from the designer/
  inspector.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields (section labels, hints) in
  **all six** languages.
- `crates/cobolt-ide/src/app.rs` — open/route the COBOL Structure editor; mark
  form dirty + regenerate on Build/Run/Debug/Check.
- `docs/developers-guide-en.md` — new section (COBOL Structure + GLOBAL/EXTERNAL
  + Rust FFI). `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor bump.

## 3. Data / model changes

- **`.cfrm` schema:** new optional `<cobol-structure>` element with one child per
  block (CDATA). Missing → empty. `user_ws_source` migrates to
  `working_storage` (read old attribute as a fallback). Backward compatible.
- **AST:** `EnvironmentDivision.repository: Vec<RepositoryEntry>`;
  `Usage::ObjectRef(String)`. Data decls already carry `global`/`external` flags
  (data.rs:335–341). User procedures are raw-COBOL nested programs (no new AST —
  they go through the normal program parser).
- **Runtime:** `ExternalStore` (run-unit singleton, `Arc<Mutex<…>>`), threaded
  through `Interpreter`/`Environment` construction so every form interpreter in a
  project shares the same instance (IDE: one per running project; compiled
  binary: one process-global). *(Phase 2)* `RustBridge` + handle table.

## 4. Key decisions & alternatives

- **One raw-COBOL block per section (dev writes clauses).** Why: extends the
  proven `user_ws_source` pattern; maximal COBOL flexibility; minimal new UI.
  Rejected: a structured per-item grid with GLOBAL/EXTERNAL checkboxes (more UI,
  less expressive). (Q1.)
- **EXTERNAL = shared `Arc<Mutex>` store by real name; GLOBAL stays per-form via
  interpreter isolation.** Why: isolation already namespaces GLOBAL; only
  EXTERNAL needs cross-form sharing — far less invasive than unifying all
  storage and mangling every name. Rejected: single global namespace + literal
  `form-name.item` mangling.
- **Phase the Rust FFI after structure+sharing.** Why: FFI (marshaling,
  ownership, safety) is the deepest/riskiest part. Rejected: all-in-one. (Q2.)
- **Curated/registered Rust bridge**, not auto-binding arbitrary crates. Why:
  safety and a tractable first cut; arbitrary dynamic Rust loading is unsafe and
  huge. Rejected: generic crate FFI.
- **Include per-form user procedures, woven as nested programs.** Each user
  procedure becomes a nested program sibling to the event-handler nested
  programs, callable by name (reusing the event loop's nested-program CALL
  dispatch) and seeing the form's GLOBAL data. Why: the operator wants the
  PowerCOBOL "User Procedure" capability now; nested-program reuse is the
  lowest-friction fit. Rejected: PROCEDURE DIVISION paragraphs (not reachable
  from the separate nested handler programs). (Q4.)
- **Drop BASED-STORAGE and CONSTANT** (Q5) — removed from sections, lexer, AST,
  parser, and codegen scope.

## 5. Risks & mitigations

- **COBOL-2002 parser/semantic is real language work.** → Incremental, one
  construct at a time, each with focused tests; reuse existing section-dispatch.
- **EXTERNAL store concurrency + description mismatch.** → `Mutex`; on first
  registration record the description; on mismatch emit a diagnostic (don't
  silently share incompatible layouts).
- **Rust FFI safety** (panics/UB across the boundary, leaks). → Curated registry
  only; wrap calls in `catch_unwind`; explicit handle table with a defined drop;
  no arbitrary crate loading in the first cut.
- **User-procedure CALL semantics** — strict COBOL would require `IS COMMON` for
  a nested program to be CALLed by a sibling. → Verify the runtime's
  nested-program dispatch resolves user procedures by name like it does event
  handlers; if it needs `COMMON`, emit the user-procedure nested programs with
  the `COMMON` attribute.
- **`.cfrm` backward compatibility.** → Optional/defaulted elements; migrate
  `user_ws_source`.
- **Weaving correctness / column-72.** → Weave verbatim (dev owns formatting);
  validate by building a sample 2-form project; reuse the examples' build check.

## 6. Test strategy

- **Lexer/parser unit (`cobolt-lexer`, `cobolt-parser`):** tokens + parse for
  `REPOSITORY`, `BASED-STORAGE`, `CONSTANT`, `USAGE OBJECT REFERENCE`; **report**
  pass/fail. Round-trip a program containing each.
- **Semantic (`cobolt-semantic`):** `EXTERNAL` on a `05` item → diagnostic;
  mismatched EXTERNAL descriptions → warning. Assert the diagnostics fire.
- **Runtime integration (`cobolt-runtime`):** two interpreters sharing one
  `ExternalStore` — set `EXTERNAL WS-COUNTER` in program A, read the same value
  in program B; assert equality. GLOBAL item readable from a nested handler;
  not visible to a second form's interpreter.
- **Codegen (`cobolt-codegen`):** generated program contains each woven block in
  the correct division/section, under the banner; builds via `rcrun`.
- **User procedures (`cobolt-codegen` + `cobolt-runtime`):** a form with a user
  procedure that updates a GLOBAL item, CALLed from an event handler — the
  handler sees the updated value; assert it builds and the call resolves.
- **Rust FFI (Phase 2):** register `Rust.String`; `05 S USAGE OBJECT REFERENCE
  RUST-STRING`; `INVOKE S "len"` returns the right length; a drop counter proves
  the object is released (no leak).
- **Manual/visual:** open the COBOL Structure editor; build a 2-form sample that
  shares an `EXTERNAL` counter; run both and watch the shared value; confirm a
  `GLOBAL` item is usable in a handler.

## 7. Steering compliance

- [ ] i18n: all new COBOL-Structure editor strings in 6 languages (`i18n.rs`).
- [ ] Generated-code banner + regenerate-on-Build/Run/Debug/Check preserved;
  blocks woven into the generated outer program; `.cbl` stays a build artifact.
- [ ] English dev guide updated (new section); translations untouched.
- [ ] Fix vs feature: **feature** → minor (`y`) bump + `CHANGELOG.md`.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/source English.
