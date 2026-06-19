# Tasks — Form module model & project organization

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-19

Ordered, independently-verifiable tasks by the plan's phases (§8). The project
stays green after each task. **R11 (`INVOKE-FORM`) is deferred** — not in scope.
R1/R2/R3/R5/R7 are already satisfied (codegen + spec 005) — no tasks. **R10 changes
nested-program call semantics; `INITIALIZE` is unchanged** (developer's tool).

## Phase 1 — `IS COMMON` for every procedure

- [x] **T1 — Emit `IS COMMON` on event handlers too** (R4; AC2)
  - Files: `crates/cobolt-codegen/src/lib.rs` (`write_nested_programs`: pass
    `common = true` for the form-event and per-control-event
    `write_nested_program(...)` calls, matching user procedures).
  - Do: every woven procedure (event + user) gets `IS COMMON PROGRAM`.
  - Verify: `cargo test -p cobolt-codegen` — golden/unit: a form with two event
    handlers + one user procedure emits **three** `PROGRAM-ID … IS COMMON PROGRAM`.
    `cargo test -p cobolt-runtime` — a handler that `CALL`s another handler in the
    same form runs (new test). Report counts.

## Phase 2 — Static-by-default procedure lifecycle

- [x] **T2 — Persistent per-program local store** (R10; AC5)
  - *Note: surfaced a pre-existing `INITIALIZE` limitation — it does not resolve
    nested-program-local decls, so `INITIALIZE` won't reset a procedure local
    today. Left as-is (operator direction); documented via an `#[ignore]`d test.*
  - Files: `crates/cobolt-runtime/src/interpreter.rs` — add
    `program_state: HashMap<String, Vec<(String, CobolValue)>>`; in `exec_call`'s
    nested branch (~3531–3585) materialise `local_items` **once** on first CALL,
    **reuse** thereafter (don't pop on `GOBACK`); keep LINKAGE copy-in/out per
    call; in `exec_cancel` (~3204) **remove** the program's entry (genuine reset).
    Leave `INITIALIZE` untouched.
  - Do: nested-program local WORKING-STORAGE persists across calls (static); state
    survives re-entry, is not cancelled on exit, and resets on `CANCEL`.
  - Verify: `cargo test -p cobolt-runtime` — a procedure increments a local
    counter across three CALLs → `1,2,3`; after `CANCEL` → `1` again (new test).
    Then run the **full** suite `cargo test -p cobolt-runtime`; fix any existing
    nested-program test that implicitly relied on per-call re-init (e.g. add an
    explicit `INITIALIZE`). Report counts + list any tests changed.

## Phase 3 — GLOBAL `FD` + procedure-local privacy

- [x] **T3 — Accept/validate `GLOBAL` on `FD` (and reject misuse)** (R6)
  - Files: `crates/cobolt-parser`/`crates/cobolt-ast` (ensure the `GLOBAL` clause
    on an `FD` parses into the AST — add a flag if missing);
    `crates/cobolt-semantic/src/` (`external.rs` or new `global.rs` + `lib.rs`
    wiring): `GLOBAL` valid on `FD`/`01`/`77`; a diagnostic when on a subordinate
    item.
  - Verify: `cargo test -p cobolt-parser` (FD GLOBAL parses); `cargo test -p
    cobolt-semantic` — `GLOBAL` on a `05` item → diagnostic; on `FD`/`01`/`77` →
    clean. Report counts.

- [x] **T4 — GLOBAL `FD` runtime visibility + procedure-local privacy** (R6, R9; AC3)
  - *Note: privacy is enforced by the leaf-nested-program model (verified by test);
    the optional R9 informational diagnostic was not added (behaviour proven, would
    add noise).*
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (only if the test below
    fails — confirm a `GLOBAL` file opened in the form program is reachable by a
    nested procedure via the shared file map); `crates/cobolt-semantic/src/`
    (informational diagnostic: `GLOBAL` inside a leaf user/event procedure shares
    nothing outward — R9).
  - Verify: `cargo test -p cobolt-runtime` — (R6) a `GLOBAL FD` opened in the form
    program is read by a user procedure; (R9) a procedure-local item declared
    `GLOBAL` is **not** visible to a sibling procedure (two new tests). Report.

## Phase 4 — `#INCLUDE` + "New" authoring path

> ⛔ **DEFERRED (operator decision).** T5–T7 (`#INCLUDE` of external embedded
> programs + its validation + IDE affordance) are deferred, like R11. The "New"
> encapsulation is effectively the existing **Add procedure** flow (each user
> procedure already becomes a uniquely-named embedded program). `#INCLUDE` needs a
> base path threaded into the currently-pure `codegen::generate` across ~6 call
> sites + model/XML + validation + IDE/i18n; revisit in a follow-up.

- [ ] **T5 — Weave-time `#INCLUDE` expander** (R12; AC7) — **DEFERRED**
  - Files: `crates/cobolt-codegen/src/lib.rs` (recognise a `#INCLUDE "file"`
    directive in the procedures area; resolve **project-relative**, read, and
    inline the file's embedded programs into the nested-program region with a
    provenance banner comment). Optional `Form.includes` model field — decide while
    implementing; if added, also `crates/cobolt-forms/src/{model.rs,xml.rs}`.
  - Verify: `cargo test -p cobolt-codegen` — a form with a `#INCLUDE` emits the
    referenced embedded program(s) under the nested-program banner; a missing file
    yields a build diagnostic (no panic). Report counts.

- [ ] **T6 — Check-time validation + raw-code flag** (R12; AC7, Q4) — **DEFERRED**
  - Files: `crates/cobolt-semantic/src/` (+ `lib.rs` wiring): each `#INCLUDE`d
    program has a unique `PROGRAM-ID` and a terminating `END PROGRAM`; no
    procedure-name collisions; a **diagnostic** when a user-procedure body is raw
    paragraph code lacking its own `PROGRAM-ID` (flag, not block).
  - Verify: `cargo test -p cobolt-semantic` — duplicate/missing `PROGRAM-ID`,
    missing `END PROGRAM`, and a name collision each produce a diagnostic;
    well-formed includes pass clean; a raw-code procedure is flagged. Report.

- [ ] **T7 — IDE "New" labelling + `#INCLUDE` affordance + i18n** (R12, R13) — **DEFERRED**
  - Files: `crates/cobolt-ide/src/panels/{properties.rs,designer.rs}` (label the
    add-procedure action **"New"**; surface a `#INCLUDE` entry in the COBOL
    Structure procedures area; show validation diagnostics);
    `crates/cobolt-ide/src/i18n.rs` (`Tr` fields ×6: "New", "#INCLUDE", the
    include/raw-code diagnostics).
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n` (×6, no
    empty). Manual: "New" creates a uniquely-named embedded procedure; a bad
    `#INCLUDE` shows the diagnostic.

## Phase 4b — Inline `::` as a value operand (independent)

- [x] **T8 — Evaluate `Expr::MethodCall` in value position** (R16; AC9)
  - *Parser + evaluator already produced/handled `Expr::MethodCall`; the real gap
    was that the inline form arrives with the method uppercased (`LEN`) by the
    lexer while the bridge methods are lowercase — fixed by lowercasing the method
    in `invoke_rust`. `DISPLAY S::len()` and `MOVE S::len() TO N` now yield 5.*
  - Files: `crates/cobolt-parser` (confirm `obj::method()` parses in **operand**
    position inside `DISPLAY`/`MOVE`/`COMPUTE`; add a thin parse path if `::` is
    currently statement-only); `crates/cobolt-runtime/src/interpreter.rs` (add an
    `Expr::MethodCall` arm to the expression evaluator, dispatching through the
    existing `invoke_rust`/`exec_method` path and marshaling the result to a
    `CobolValue`).
  - Do: the 005 AC6 inline demo works; `INVOKE … RETURNING` stays valid.
  - Verify: `cargo test -p cobolt-runtime` — for `01 S USAGE OBJECT REFERENCE
    RUST-STRING VALUE "hello"`: `DISPLAY S::len()` outputs `5`; `MOVE S::len() TO
    N` sets `N = 5`; object dropped, no leak (new test). Report counts.

## Phase 5 — Docs & finalize

- [x] **T9 — Docs (English guide)** (R14; AC8)
  - Files: `docs/developers-guide-en.md` (extend §21 / COBOL Structure & shared
    data: the form-module model — one program per form, `IS COMMON` on all
    procedures, `GLOBAL` `FD`, the **static** procedure lifecycle, the
    "New"/`#INCLUDE` authoring path, and inline `obj::method()` as a value).
  - Verify: review; English guide only — translations untouched.

- [x] **T10 — Finalize** (all in-scope ACs) — version 1.27.0 + CHANGELOG; full
      `cargo test --workspace` green; i18n ×6 green.
  - Files: `crates/cobolt-ide/src/version.rs` (+ `CHANGELOG.md`) — feature minor
    bump; `specs/steering/{product.md,tech.md}` if the model note warrants it.
  - Verify: `cargo build --workspace` + `cargo test --workspace` green;
    `cargo test -p cobolt-ide i18n`. Manual AC walkthrough: AC1 (two forms → two
    modules), AC2 (all procedures `IS COMMON` + handler→handler CALL), AC3
    (`GLOBAL`/`EXTERNAL` data + GLOBAL `FD`), AC4 (`EXTERNAL` cross-form, same
    run unit — R8 cross-process deferred), AC5 (static + `CANCEL` reset), AC7
    ("New" + `#INCLUDE` + Check diagnostics), AC8 (i18n ×6 + docs), AC9
    (`DISPLAY S::len()`). **AC6 (`INVOKE-FORM`) is deferred with R11.**

## Done criteria
All in-scope acceptance criteria are covered (AC1: T1 · AC2: T1 · AC3: T3/T4 ·
AC4: existing 005 store, re-verified in T10 · AC5: T2 · AC7: T5/T6/T7 · AC8:
T9/T10 · AC9: T8), tests pass, `INITIALIZE` is unchanged, Liquid-Glass/desktop
behaviour is unaffected, docs/steering updated, and the work is committed as
feature commit(s) per the operator's rules (do **not** commit/push unless asked).
**AC6 / R11 (`INVOKE-FORM`) and cross-process `EXTERNAL` (R8) remain deferred.**
