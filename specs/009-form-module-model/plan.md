# Plan — Form module model & project organization

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-19

## 1. Approach

Spec 009 records a model that is **mostly already realised** (R1/R2/R3/R5/R7) by
the nested-program codegen and spec 005's run-unit `EXTERNAL` store. This plan
implements the **active gaps** with the smallest, most faithful changes to the
existing pipeline (`cobolt-codegen` → `cobolt-parser`/`-ast` → `cobolt-semantic`
→ `cobolt-runtime`, surfaced in `cobolt-ide`). **R11 (`INVOKE-FORM`) is deferred**
and not touched here.

- **R4 — `IS COMMON` on every procedure.** Flip the one call site that emits
  event handlers to pass `common = true`, matching user procedures. Today
  `write_nested_programs` calls `write_nested_program(..., false)` for events and
  `(..., true)` for user procedures (lib.rs:1047/1054/1064). Making **all** woven
  procedures `IS COMMON` lets any handler/procedure `CALL` any other within the
  form module (valid COBOL-85 for sibling contained programs). The runtime already
  resolves CALLs by program-id through a flat `nested_registry`, so no runtime
  change is needed for visibility.

- **R6 — `FD … IS GLOBAL`.** The FILE SECTION is woven verbatim (lib.rs:173), so a
  developer can already write `FD F IS GLOBAL`. The work is (a) **semantic**: allow
  `GLOBAL` on `FD` (parser/AST already carry the FD; ensure the `GLOBAL` clause is
  accepted and not flagged), and (b) **runtime**: a `GLOBAL` file declared in the
  outer/form program is visible to its nested procedures — files live in the
  interpreter's shared `open_files`/file-control map keyed by name, so a nested
  program performing I/O on a GLOBAL file name already reaches the same handle.
  Add a focused test to lock the behaviour; add validation that `GLOBAL` on a
  non-`FD`/non-`01`/`77` is a diagnostic (extends `external.rs` → rename intent to
  cover both clauses, or a sibling `global.rs`).

- **R9 — procedure-local privacy.** Already true structurally: each procedure is a
  **leaf** nested program (no further nesting), so a `GLOBAL` clause on its own
  data shares nothing outward. Lock it with a semantic check (a `GLOBAL` inside a
  user/event procedure body is a no-op and should emit an *informational* warning,
  not silently mislead) plus a runtime test proving isolation.

- **R10 — static-by-default lifecycle.** *The real behavioural change.* Today
  `exec_call` pushes a **fresh** `local_items` scope per call and pops it on return
  (interpreter.rs:3555/3579), re-initialising local WS every entry. Change to a
  **persistent per-program instance store**: the first CALL of a program
  materialises its `local_items` into a `program_state: HashMap<String, Vec<(String,
  CobolValue)>>` keyed by program-id; subsequent CALLs **reuse** that store
  (preserving values); `GOBACK` does **not** discard it. `CANCEL` removes the
  entry (next call re-initialises) — `exec_cancel` already re-applies `local_items`,
  so it becomes the genuine reset path. LINKAGE params still bind per call. This
  makes procedures static exactly as COBOL-85 specifies.
  > **`INITIALIZE` is unchanged and out of this scope.** It already works and is
  > the **developer's** tool for deciding how to handle persistent values between
  > calls to a procedure — we neither add nor modify it. R10's only change is
  > making local WORKING-STORAGE persist across calls (static); whether to reset
  > any item on entry stays a developer decision via `INITIALIZE`.

- **R12 — Procedure-Division authoring (`#INCLUDE` + "New").** "New" already maps
  to the existing **Add procedure** flow (each user procedure becomes a uniquely
  named embedded program via `write_nested_program`). Add: (a) a **`#INCLUDE
  "file"`** directive recognised at **weave time** in codegen — the file's contents
  (one or more `IDENTIFICATION DIVISION … END PROGRAM` embedded programs) are
  inlined among the form's nested programs; (b) **Check-time validation** that each
  included program has a unique `PROGRAM-ID` and a terminating `END PROGRAM`, and
  that procedure names don't collide; (c) a **diagnostic** when a user procedure
  body looks like raw paragraph code lacking its own `PROGRAM-ID` (the "don't add
  raw code outside New" guard — flag, not hard block, per Q4).

- **R16 — inline `obj::method()` as a value operand.** The parser already models
  the inline call as `Expr::MethodCall` (tasks #47/#48) and dispatches it as a
  *statement*; the runtime's expression evaluator does **not** yet handle
  `Expr::MethodCall` in value position (test_rust_ffi.rs:83-85 documents this).
  Add an `Expr::MethodCall` arm to the interpreter's expression evaluator that
  routes through the **same** method dispatcher the `INVOKE` statement uses
  (universal / per-widget / Rust-bridge — `invoke_rust` / `exec_method`), marshals
  the result to a `CobolValue`, and yields it in place. First confirm the parser
  accepts `S::len()` in operand position (inside `DISPLAY`/`MOVE`/`COMPUTE`); add a
  thin parse path if `::` is currently accepted only as a statement. The
  `INVOKE … RETURNING` form stays valid (unchanged).

## 2. Affected crates / files

- `crates/cobolt-codegen/src/lib.rs`
  - R4: `write_nested_programs` — pass `common = true` for form events and
    per-control events (the two `write_nested_program(..., false)` calls).
  - R12: a weave-time `#INCLUDE` expander (resolve project-relative path, read,
    inline the embedded programs into the nested-program region); emit a banner
    comment noting the include source.
- `crates/cobolt-semantic/src/` (`external.rs` + new `global.rs` or extend)
  - R6: accept `GLOBAL` on `FD`/`01`/`77`; diagnostic otherwise.
  - R9: informational diagnostic for `GLOBAL` inside a leaf procedure.
  - R12: validate `#INCLUDE`d programs (unique `PROGRAM-ID`, `END PROGRAM`); flag
    raw-code user procedures; wire into the Check/Build pass (lib.rs).
- `crates/cobolt-runtime/src/interpreter.rs`
  - R10: add `program_state` (per-program persistent local store); rework
    `exec_call`'s nested branch (≈3531–3585) to materialise-once / reuse / not-pop;
    update `exec_cancel` (≈3204) to drop the persistent entry; keep LINKAGE
    copy-in/out per call.
  - R6: confirm GLOBAL-file visibility from nested programs (likely test-only).
  - R16: add an `Expr::MethodCall` arm to the expression evaluator, dispatching via
    the existing `invoke_rust`/`exec_method` path and returning a `CobolValue`.
- `crates/cobolt-parser` / `crates/cobolt-ast`
  - R6: ensure `GLOBAL` clause on `FD` parses into the AST (add a flag if missing).
  - R12: `#INCLUDE` is handled in codegen (text weave), **not** the COBOL parser,
    so no grammar change unless we choose to model it in the AST (we don't — see
    Decisions).
  - R16: confirm `obj::method()` parses in **operand** position (DISPLAY/MOVE/
    COMPUTE); add a thin parse path if it is currently statement-only.
- `crates/cobolt-ide/src/`
  - `panels/properties.rs` / `panels/designer.rs`: label the user-procedure
    action as **"New"**; surface the `#INCLUDE` affordance in the COBOL Structure
    procedures area; show validation diagnostics.
  - `i18n.rs`: new `Tr` fields ×6 ("New procedure", "#INCLUDE", the raw-code and
    include diagnostics).
- `docs/developers-guide-en.md`: extend §21 (COBOL Structure & shared data) with
  the form-module model — `IS COMMON`, GLOBAL `FD`, the static lifecycle, and the
  "New"/`#INCLUDE` authoring path.
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs`: feature minor bump.

## 3. Data / model changes

- **No `.cfrm` schema change required** for R4/R6/R9/R10. `#INCLUDE` (R12) is just
  text inside an existing editable block (a user procedure body or a dedicated
  "includes" line in the COBOL Structure) — additive, no new field strictly
  needed; an optional `Form.includes: Vec<String>` could make it first-class
  (decide in /tasks).
- **Runtime:** new `program_state: HashMap<String /*PROG-ID*/, Vec<(String,
  CobolValue)>>` on the interpreter (persistent local WS per program). No public
  API change.
- **No generated `.cbl` contract change:** banner + regenerate-on-action preserved;
  `#INCLUDE` expansion happens during generation, so the emitted `.cbl` remains a
  self-contained build artifact.

## 4. Key decisions & alternatives

- **`IS COMMON` on all procedures (R4).** — Why: the spec's "callable from
  anywhere in the form module" + uniformity; COBOL-85 requires COMMON for a
  sibling contained program to be CALLed. Rejected: keeping events non-COMMON
  (blocks handler→handler calls; asymmetric with user procedures).
- **Persistent per-program store for static lifecycle (R10).** — Why: faithful
  COBOL-85 semantics (a called program keeps state until CANCEL/INITIAL). Rejected:
  the current push/pop-per-call (re-inits every entry — wrong); and a global
  always-on overlay (would leak locals across *different* programs and break R9).
- **`#INCLUDE` expanded at weave time in codegen, not in the COBOL grammar (R12).**
  — Why: it's a RAD/preprocessor convenience that inlines whole embedded programs;
  keeping it out of `cobolt-parser` avoids polluting the language and keeps the
  emitted `.cbl` standard. Rejected: a real COBOL `COPY`/parser directive (heavier;
  the included unit here is *whole programs*, not a copybook fragment).
- **Q1 (cross-executable `EXTERNAL`, R8) — RESOLVED: scope to a single run unit
  now.** Implement/keep `EXTERNAL` sharing across **CALLed modules within one
  process** (standard COBOL run unit; already done in 005). True cross-*process*
  executable sharing needs OS shared memory/IPC, has **no portable web/WASM
  equivalent** (R15), and isn't required by any current workflow → **deferred** as
  a future, desktop-only capability. The spec's R8 wording is satisfied for the
  run-unit case; cross-process is recorded as out-of-scope-now.
- **Q3 (`#INCLUDE` resolution) — RESOLVED:** project-relative path; expanded into
  the nested-program region (so included programs are siblings and are
  `IS COMMON`-eligible); validated at Check.
- **Q4 ("New" vs raw code) — RESOLVED:** a **Check-time diagnostic** (flag), not a
  hard editor block — matches R12's "shall be flagged" and avoids fighting the
  developer.

## 5. Risks & mitigations

- **R10 changes call semantics broadly.** Persisting local WS could surprise
  programs that assumed re-init. → It *is* the correct COBOL-85 behaviour; gate
  with focused before/after tests (value survives re-entry; `CANCEL` resets;
  `INITIALIZE` resets) and run the **full** runtime suite to catch regressions in
  existing nested-program tests (some may have implicitly relied on re-init —
  fix them to use `INITIALIZE`).
- **`IS COMMON` on events might change CALL resolution.** → COMMON only *widens*
  visibility; the flat `nested_registry` already resolves by id. Add a test:
  handler A CALLs handler B successfully.
- **GLOBAL `FD` visibility in the interpreter.** May already work (shared file
  map) or may need a small lookup fix. → Write the test first; implement only if
  it fails.
- **`#INCLUDE` path traversal / missing file.** → Resolve only within the project
  root; a missing or unreadable include, or a bad embedded program, is a **build
  diagnostic**, never a panic or silent skip.
- **Generated-code contract.** `#INCLUDE` expansion must keep the banner and the
  regenerate-on-action flow. → Expand during `generate`, emit a provenance comment,
  cover with a golden test.

## 6. Test strategy

- **`cobolt-codegen`:** golden test — a form with two event handlers + one user
  procedure emits **all three** with `IS COMMON` (R4). Golden test — a `#INCLUDE`
  directive inlines the referenced embedded program(s) under the nested-program
  banner (R12). Report counts.
- **`cobolt-runtime`:** (R10) a nested program increments a local counter across
  three CALLs → observes `1,2,3` (static); after `CANCEL` → back to `1`.
  (`INITIALIZE` is unchanged — its existing tests remain the developer-reset
  coverage; no new INITIALIZE behaviour is added.) (R4) handler→handler CALL
  works. (R6) a
  GLOBAL `FD` opened in the form program is readable from a user procedure. (R9) a
  procedure-local item with `GLOBAL` is invisible to a sibling procedure. (R16)
  `DISPLAY S::len()` for `01 S USAGE OBJECT REFERENCE RUST-STRING VALUE "hello"`
  outputs `5`, and `MOVE S::len() TO N` sets `N = 5` (the 005 AC6 inline demo).
  Report pass counts.
- **`cobolt-semantic`:** `GLOBAL` on a `05` item → diagnostic; on `FD`/`01`/`77` →
  clean (R6). A `#INCLUDE`d file with a duplicate or missing `PROGRAM-ID` /
  missing `END PROGRAM` → diagnostic; well-formed → clean (R12). A raw-code user
  procedure → flag (R12/Q4). Report counts.
- **`cobolt-ide`:** `cargo test -p cobolt-ide i18n` (×6, no empty). Build check.
- **Manual:** in the IDE, define two handlers where one calls the other; add a
  user procedure with a persistent counter and watch it accumulate across clicks;
  add a `#INCLUDE` and Build → see the included program in the generated `.cbl`;
  break an include (dup PROGRAM-ID) → see the Check diagnostic.

## 7. Steering compliance

- [ ] i18n: new UI strings in 6 languages (R13).
- [ ] Generated-code banner + regenerate-on-action preserved; `#INCLUDE` expands
      at generation; emitted `.cbl` stays a self-contained artifact (no hand-edit).
- [ ] English dev guide updated (§21 extension); translations untouched (R14).
- [ ] Fix vs feature: **feature** → minor bump + CHANGELOG.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/source English.
- [ ] Portability (006): R8 scoped to a single run unit so the model stays
      wasm-portable; cross-process sharing deferred.

## 8. Phasing (proposed for /tasks)

- **Phase 1 — `IS COMMON` for all procedures (R4).** Codegen one-line-ish change +
  golden test + handler→handler runtime test. Lowest risk, immediate correctness.
- **Phase 2 — Static lifecycle (R10).** Interpreter `program_state` rework +
  `CANCEL`/`INITIALIZE` semantics; before/after tests; full runtime suite green
  (fix any tests that relied on re-init).
- **Phase 3 — GLOBAL `FD` + procedure-local privacy (R6, R9).** Semantic
  validation + runtime visibility tests.
- **Phase 4 — `#INCLUDE` + "New" + validation (R12).** Weave-time expander,
  Check-time validation, IDE labelling/affordance + i18n.
- **Phase 4b — Inline `::` as a value operand (R16).** Parser operand-position
  check + interpreter `Expr::MethodCall` evaluation; 005 AC6 inline demo test.
  Independent of the form-module phases — can land any time.
- **Phase 5 — Docs + finalize.** Dev-guide §21 extension, version bump/CHANGELOG,
  full workspace test, AC walkthrough (AC1–AC5, AC7, AC8, AC9; AC6 form-invocation
  remains deferred with R11).
