# Tasks — Form COBOL Structure, shared data (GLOBAL/EXTERNAL), and Rust FFI

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-18

Ordered, small, independently-verifiable tasks. Each names files, the
requirement(s) it satisfies, and how to verify. Build & test each touched crate
before moving on. **Do not commit/push until the operator asks.**

## Phase 1 — COBOL Structure + user procedures + GLOBAL/EXTERNAL sharing

- [x] **T1 — Form model + `.cfrm` persistence** (R1, R2, R3a)
  - Files: `crates/cobolt-forms/src/model.rs` (add `CobolStructure {
    special_names, repository, file_control, file_section, working_storage:
    String }` and `user_procedures: Vec<UserProcedure { name, code }>` to `Form`;
    migrate `user_ws_source` → `working_storage`), `crates/cobolt-forms/src/xml.rs`.
  - Do: persist the new blocks + user procedures as optional `.cfrm` elements
    (CDATA); old `.cfrm` (incl. `user_ws_source`) still loads (compat fallback).
  - Verify: `cargo test -p cobolt-forms` green; a round-trip test saves+loads a
    form with all blocks + a user procedure and asserts equality; an old `.cfrm`
    still loads with `user_ws_source` mapped into `working_storage`.

- [x] **T2 — Codegen weaving** (R3, R3b)
  - Files: `crates/cobolt-codegen/src/lib.rs`.
  - Do: weave each block into its division/section in order
    (ENV→CONFIG→`SPECIAL-NAMES`/`REPOSITORY`; ENV→I-O→`FILE-CONTROL`;
    DATA→`FILE`/`WORKING-STORAGE`), verbatim, under the banner; emit each user
    procedure as a nested program (sibling to event-handler nested programs).
  - Verify: `cargo test -p cobolt-codegen` green; a test asserts the generated
    program contains each non-empty block in the correct division/section + the
    banner + a user-procedure nested program; the generated `.cbl` parses
    (`rcrun check`).

- [x] **T3 — Lexer/AST/parser: REPOSITORY + USAGE OBJECT REFERENCE** (R4, R5)
  - Files: `crates/cobolt-lexer/src/{keywords.rs,token.rs}` (`REPOSITORY`,
    `OBJECT`), `crates/cobolt-ast/**` (`repository` entries, `Usage::ObjectRef`),
    `crates/cobolt-parser/src/{parser.rs,data.rs}`.
  - Do: parse a `REPOSITORY` paragraph (name ↦ external string) in the
    CONFIGURATION SECTION and `nn NAME USAGE OBJECT REFERENCE <name>` on data
    items (parse-only; Rust semantics land in Phase 2). Invalid content → a clear
    diagnostic, not a panic.
  - Verify: `cargo test -p cobolt-lexer -p cobolt-parser` green; round-trip a
    program with `REPOSITORY` + an `OBJECT REFERENCE` item; `rcrun check` passes.

- [x] **T4 — Semantic: EXTERNAL validation** (R10)
  - Files: `crates/cobolt-semantic/**`.
  - Do: diagnose `EXTERNAL` on non-`01`/`77`/`FD` items; warn when two `EXTERNAL`
    declarations of the same name have differing descriptions.
  - Verify: `cargo test -p cobolt-semantic` green; tests assert the diagnostic
    fires on a `05 … EXTERNAL` and the warning on a description mismatch.

- [ ] **T5 — Runtime EXTERNAL store** (R6, R7, R8, R9)
  - Files: `crates/cobolt-runtime/src/{environment.rs,interpreter.rs}`,
    `crates/cobolt-ide/src/form_runtime.rs`, `crates/cobolt-compiler/**` (thread
    the store into the compiled binary's run unit).
  - Do: a run-unit-wide `Arc<Mutex<HashMap<String, CobolValue>>>`; register
    `EXTERNAL` (and `GLOBAL EXTERNAL`) 01/77/FD items by real name on program
    init; route their reads/writes to the store. GLOBAL-only stays per-form
    (interpreter isolation — no mangling).
  - Verify: `cargo test -p cobolt-runtime` green; an integration test runs two
    interpreters sharing one store — program A sets `EXTERNAL WS-COUNTER`,
    program B reads the same value; a GLOBAL item in A is **not** visible to B.

- [ ] **T6 — User-procedure CALL** (R3b, AC8)
  - Files: `crates/cobolt-codegen/src/lib.rs` (emit `COMMON` if required),
    `crates/cobolt-runtime/src/interpreter.rs` (nested-program CALL resolution).
  - Do: ensure an event handler can `CALL "<user-proc>"`; if the runtime needs
    `IS COMMON` for sibling nested-program calls, emit user procedures `COMMON`.
  - Verify: `cargo test -p cobolt-runtime` green; a test: handler CALLs a user
    procedure that updates a GLOBAL item; the handler then reads the new value.

- [ ] **T7 — IDE COBOL Structure editor + i18n** (R1, R3a, R11, R12)
  - Files: `crates/cobolt-ide/src/panels/cobol_structure.rs` (new),
    `crates/cobolt-ide/src/app.rs` (open/route, dirty + regenerate),
    `crates/cobolt-ide/src/i18n.rs` (new `Tr` fields ×6 languages).
  - Do: an editor with one COBOL code-editor block per section + a
    user-procedures list (add/rename/edit). Reuse the existing code editor.
    Developer writes `GLOBAL`/`EXTERNAL` clauses themselves (Q1). Regenerate on
    Build/Run/Debug/Check.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide i18n` green
    (no empty translations); manual: edit each block + add a user procedure, save,
    Build → blocks appear in the generated `.cbl`.

- [ ] **T8 — Phase 1 docs + version/CHANGELOG** (AC1, AC7)
  - Files: `docs/developers-guide-en.md` (new "COBOL Structure & shared data"
    section: the editor, GLOBAL/EXTERNAL/GLOBAL EXTERNAL, user procedures),
    `crates/cobolt-ide/src/version.rs` (minor bump), `CHANGELOG.md`.
  - Verify: `cargo build --workspace` + `cargo test --workspace` green; manual
    end-to-end: a 2-form sample sharing an `EXTERNAL` counter shows the shared
    value at run time (AC3); a `GLOBAL` item is usable in a handler (AC2);
    `GLOBAL EXTERNAL` works both ways (AC4). English guide only; banner preserved.

## Phase 2 — Rust FFI

- [ ] **T9 — Rust bridge registry + handles** (R13, R16, R17)
  - Files: `crates/cobolt-runtime/**` (new `RustBridge`: repo-name ↦
    constructor/method table for a curated first-cut type set — `i32/i64/f64/
    bool`, `String`, `Vec`; an object handle table id ↦ `Box<dyn Any>` with a
    defined drop). Wrap calls in `catch_unwind`.
  - Verify: `cargo test -p cobolt-runtime` green; unit tests create a `Rust.String`
    handle, call a method, and assert a drop counter reaches zero (no leak).

- [ ] **T10 — INVOKE/`::` dispatch into Rust** (R13, R14, R15, AC6)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (resolve `OBJECT REFERENCE`
    items to handles; route `INVOKE NAME "method"` / `NAME::method()` to the
    bridge, marshaling args/results), `crates/cobolt-semantic/**` (resolve
    `REPOSITORY` bindings to types).
  - Verify: `cargo test -p cobolt-runtime` green; the AC6 demo program —
    `REPOSITORY` binds `RUST-STRING` ↦ `Rust.String`; `05 S USAGE OBJECT
    REFERENCE RUST-STRING`; `INVOKE S "len"` (and `S::len()`) returns the right
    length; the object is dropped with no leak. `rcrun build` of the demo passes.

- [ ] **T11 — Phase 2 docs + finalize** (AC6, AC7)
  - Files: `docs/developers-guide-en.md` (Rust FFI subsection),
    `crates/cobolt-ide/src/version.rs` (minor bump), `CHANGELOG.md`,
    `specs/steering/docs.md` (registry rows for the new code areas).
  - Verify: `cargo build --workspace` + `cargo test --workspace` green; the AC6
    demo builds & runs; English guide only; commits split fix/feature per the
    operator's rules (only when asked).

## Done criteria
All acceptance criteria in `spec.md` (AC1–AC8) are checked, `cargo build/test
--workspace` pass, docs updated (English guide + steering registry), i18n in all
six languages, and the change is committed as a **feature** (per phase) per the
operator's rules (do **not** commit/push unless asked).
