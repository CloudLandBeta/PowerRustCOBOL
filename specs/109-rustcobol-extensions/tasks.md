<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — RustCOBOL extensions for compact generated applications

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-10-30

Ordered, small, independently-verifiable tasks. Each names the files it touches, the
requirements it satisfies and how to verify it. Check off as completed.

**Gate before T1:** spec 108 is in `main` and its port is in `2.x.x` (operator ruling
2026-10-30). Branch `features-2x`: `git rebase 2.x.x` before the first edit; fixes
found on the way go to `fixes-2x` first (rule #5). Each slice ends with its **slice
close** task (sweep, KB, i18n, version, CHANGELOG, hosts) and never mixes with a fix.

**Standing verification, every slice close:** `cargo test -p cobolt-forms --features
render`; `cargo test --no-fail-fast` on the touched crates (`cobolt-ide --bin`), every
`test result:` line read; known reds on 2.x.x reported as pre-existing; credential scan
of the staged diff (AC23). Lexer/parser slices add the NIST NC census plus every
finished module's execution pass (rule #9).

## T0 — Inventory

- [ ] **T0.1 — Control-storage dependency inventory** (R29, Q9)
  - Files: new `specs/109-rustcobol-extensions/inventory.md` (report only).
  - Do: over PowerAnalytics and every `examples/*/forms`, list each `WS-<control>`
    group/item and each generated helper paragraph that authored code, Common Code or
    nested handlers read; list dynamic-reference cases; record the frozen baseline
    (SHA-256, git blob, generator commit).
  - Verify: report re-derivable by one documented command sequence; the baseline
    hashes match spec §2. No code changed.

## S3 — Collections and `PERFORM … THROUGH`

- [ ] **T3.1 — Lexer: `=>` token** (R12)
  - Files: `crates/cobolt-lexer/src/token.rs`, tests.
  - Verify: `cargo test -p cobolt-lexer`; `=>` lexes, `<>`/`>=`/`=` unchanged.

- [ ] **T3.2 — Collection value in the interpreter** (R11, R13, R14)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (value type), `crates/cobolt-ast/src/value_methods.rs`.
  - Do: ordered collection value; OCCURS fixed and ODO extents; table assignment with
    compatibility and capacity checks, overflow diagnosed, unused occurrences initialised.
  - Verify: `cargo test -p cobolt-runtime`; new cases for AC5 extents and overflow.

- [ ] **T3.3 — Methods `Join Map Filter Sum Max SortByDescending Take`** (R11, R12, R15, R37)
  - Files: `crates/cobolt-ast/src/methods.rs` (`is_known_method`), `interpreter.rs`, `crates/cobolt-parser/src/expr.rs` (lambda `ITEM => expr`).
  - Do: one canonical spelling each; pure-call lambda check (Q3), a failing element
    fails the expression naming its position; stable non-mutating sort; `Sum` of empty
    is zero, `Max` of empty a run-time error naming the method (Q12); negative `Take` rejected.
  - Verify: `cargo test -p cobolt-ast -p cobolt-parser -p cobolt-runtime`; the dispatch
    test `every_dispatched_control_method_is_spellable_inline` goes red when any listed
    method is removed (AC22).

- [ ] **T3.4 — `Split` as collection only in collection context** (R38)
  - Files: `interpreter.rs`, `methods.rs`.
  - Verify: existing `Split(",")(2)` and unsubscripted-`Split` tests stay green; new
    cases for receiver-of-method, THROUGH source, declared-table assignment (AC5).

- [ ] **T3.5 — `PERFORM VARYING element THROUGH collection`** (R16)
  - Files: `crates/cobolt-ast/src/stmt.rs` (`PerformTarget` variant), `crates/cobolt-parser/src/stmt.rs`, `interpreter.rs`, `crates/cobolt-semantic`.
  - Do: snapshot at loop start; element is the occurrence (OCCURS), the live control
    (control collection) or a copy (computed); exists only inside the loop (Q4); nesting
    and `EXIT PERFORM [CYCLE]` behave as the other inline PERFORMs.
  - Verify: tests for nested loops, early exit, evaluation count; the three old PERFORM
    forms unchanged (AC6).

- [ ] **T3.6 — COBOL test with result block** (rule #7)
  - Files: `tests/cobol/collections/*.cbl`.
  - Verify: ONE result block (name, cases, phase ms, throughput, tally); run via `rcrun`.

- [ ] **T3.7 — Slice close S3** — standing verification + NIST census; hosts (T-H); KB tables + `build_chunked_kb`; editor completion (T-IDE); `Tr` ×6; `z` bump; CHANGELOG entry; Guide text drafted in `specs/109-…/guide-notes.md` (no `docs/` on 2.x.x). (AC5, AC6, AC22)

## S2 — Interpolation, `Format`, `FORMAT`/`LOCALE`

- [ ] **T2.1 — Shared formatter crate** (R9, R23)
  - Files: new crate under `crates/` (name fixed here), workspace `Cargo.toml`; consumers `cobolt-runtime`, `cobolt-wasm-rt`.
  - Do: number, currency, compact, currency-short, percent (ratio, Q2); locales
    pt-BR, en-US, fr-FR with pinned data and version; explicit currency or project default.
  - Verify: `cargo test -p <crate>`; fixed expected outputs for signs, zero, rounding
    boundaries, compact thresholds (AC4); `Cargo.lock` diff only the new workspace member (AC12).

- [ ] **T2.2 — Project default currency** (R8, Q2b)
  - Files: `crates/cobolt-ide/src/project_model.rs` (beside `[project] locale`), Settings `Tr` ×6.
  - Verify: manifest round-trips; absent default plus bare `currency-short` is a diagnostic.

- [ ] **T2.3 — `F"…"` lexing and parsing** (R4, R5)
  - Files: `cobolt-lexer/src/token.rs`, `cobolt-parser/src/expr.rs`, `cobolt-ast/src/expr.rs`.
  - Do: embedded expressions, single `:` option vs `::` member, `{{`/`}}`, nested
    string rule, spans for unmatched braces and bad expressions.
  - Verify: parser tests incl. `{WS-A::Trim():number}`; malformed strings name the expression (AC2).

- [ ] **T2.4 — `FORMAT [IS]` / `LOCALE [IS]` data clauses** (R6–R8, R28)
  - Files: `cobolt-parser/src/data.rs` (the clause no longer reaches the skip-with-warning path), `cobolt-ast/src/data.rs`, `cobolt-semantic`.
  - Do: recognised by position only; literal or alphanumeric-item locale, qualified
    allowed, trailing spaces ignored, case-insensitive; invalid → error.
  - Verify: a fixture declaring `01 FORMAT PIC X.` and `01 LOCALE PIC X.` compiles and
    runs unchanged; DateTimePicker/Viewer `Format` property unchanged (AC3).

- [ ] **T2.5 — Runtime: `::Format()`, interpolation, DISPLAY/STRING** (R5, R6, R10)
  - Files: `interpreter.rs`, `builtins.rs`, `methods.rs` (`Format` listed, R37).
  - Do: apply in DISPLAY, STRING, interpolation, `Format()` only; locale re-read on
    every operation; MOVE, groups, file/SQL I/O, REDEFINES never format; one evaluation, left to right.
  - Verify: counter test for evaluation order; two displays with a changed locale; stored values byte-identical (AC2–AC4).

- [ ] **T2.6 — COBOL test with result block** — `tests/cobol/format/*.cbl`, rule #7.

- [ ] **T2.7 — Slice close S2** — standing verification + NIST census (new token `F"`, clause words); WASM: formatter shared source, `rt.wasm` rebuilt (`cargo run -p cobolt-wasm --example build_rt`), differential cases; KB, `Tr`, `z`, CHANGELOG, guide notes. (AC2, AC3, AC4, AC12)

## S1 — `FUNCTION-ID`

- [ ] **T1.1 — Lexer and parser: function units** (R1)
  - Files: `cobolt-lexer/src/keywords.rs`, `cobolt-parser/src/parser.rs`, `cobolt-ast/src/program.rs`.
  - Do: `FUNCTION-ID … END FUNCTION`, LINKAGE, `USING BY VALUE|REFERENCE`, `RETURNING`, `GOBACK`.
  - Verify: `cargo test -p cobolt-parser`; standalone and project units parse.

- [ ] **T1.2 — REPOSITORY registration and project-wide visibility** (R2, Q5)
  - Files: `cobolt-codegen/src/lib.rs` (REPOSITORY parse/write-back), `cobolt-compiler`, `cobolt-cli`, `cobolt-semantic`.
  - Do: units from `src/*.cbl` visible project-wide; a name equal to an intrinsic is refused naming both.
  - Verify: arity/category diagnostics; clash diagnostic test.

- [ ] **T1.3 — Runtime: call, activation isolation, return** (R2, R3)
  - Files: `interpreter.rs` (`Expr::FunctionCall` reaches user functions).
  - Verify: numeric/text returns, nested calls, recursion, one evaluation per argument, sign/scale/length preserved, no global mailbox (AC1).

- [ ] **T1.4 — WebAssembly build-time refusal** (R23)
  - Files: `crates/cobolt-wasm`, `specs/WASM-PARITY.md` (row 4 reference).
  - Verify: a Web project using a user function fails the build naming the construct.

- [ ] **T1.5 — COBOL test with result block** — `tests/cobol/functions/*.cbl`, rule #7.
- [ ] **T1.6 — Slice close S1** (AC1, AC12)

## S4 — `Descendants`, `Controls`, `SetProperties`

- [ ] **T4.1 — Shared parent-membership walk** (R17, R19)
  - Files: `cobolt-forms` (new shared fn), callers `cobolt-ide/src/panels/designer.rs` (`cascade_ids_for`) and the renderer.
  - Verify: designer delete/copy and render tests unchanged; `engine_reference_form_parity_static_vs_faces` green.

- [ ] **T4.2 — `Descendants()` and `Controls(id…)`** (R17, R37)
  - Files: `methods.rs`, `interpreter.rs`, `cobolt-form-host`.
  - Do: root excluded, hidden included, nonvisual excluded, preorder by sibling order, references to live controls; argument order kept; missing/duplicate IDs rejected.
  - Verify: tests incl. an overlapping non-child excluded (AC7).

- [ ] **T4.3 — `SetProperties("Name" value …)`** (R18, Q11)
  - Files: `methods.rs`, `interpreter.rs`.
  - Do: values evaluated once; validate all, then write; no partial write.
  - Verify: duplicate/missing IDs, bad property, bad type → zero writes; bound notifications fire (AC8).

- [ ] **T4.4 — Slice close S4** — DataGrid tests (094, 095), `rounded-corners` checks, render feature tests (AC8).

## S8 — Direct result binding

- [ ] **T8.1 — Binding model extension** (R34, R35)
  - Files: `cobolt-forms/src/model.rs` (`DataBindingDef`, `serde(default)`).
  - Verify: old `.cfrm` load and save unchanged.
- [ ] **T8.2 — Populate/refresh from the PartiQL result set** (R26, R34)
  - Files: `cobolt-codegen` (`COBOL-DATA-BINDINGS-POPULATE`), `cobolt-runtime`.
  - Do: consume 108's result contracts (R19–R21, R31–R35); no mixed old/new rows; parameters evaluated once; paging reused.
  - Verify: grid/chart/list/repeating-group comparisons, nulls, decimals, empty, failures (AC19). Spec 108's folder unchanged (AC13).
- [ ] **T8.3 — Slice close S8** (AC13, AC19)

## S6 — Declarative layout

- [ ] **T6.1 — Fix the grammar and Web lowering** (R32, Q7) — a short decision note appended to `plan.md`, against spec 056 R9, R10 and WASM-004 R8; operator reviews before T6.2.
- [ ] **T6.2 — Persist rules and lower into `layout::solve`** — `cobolt-forms/src/{model.rs,layout/}`; cycle and conflict diagnostics with source spans.
  - Verify: Compact/Medium/Expanded at 599/600 and 1023/1024, nesting, overrides, rounding, min/max, resize repetition; a window never resizes itself (AC17).
- [ ] **T6.3 — Generator emission and Web lowering or refusal** — `cobolt-codegen`, `cobolt-web-player`.
- [ ] **T6.4 — Slice close S6** (AC17)

## S7 — Animation sequences

- [ ] **T7.1 — Fix the syntax** (R33, Q8) — decision note; `delay_ms` adds to `AFTER`; operator reviews.
- [ ] **T7.2 — Model, scheduler entries, cancel/restart/close** — `cobolt-forms/src/model.rs`, `cobolt-form-host`, `cobolt-runtime`.
  - Verify: deterministic clock; absolute offsets, equal-time order, cancel, restart, two forms, close before completion; existing timers' traces unchanged (AC18).
- [ ] **T7.3 — Generator emission** — `cobolt-codegen`.
- [ ] **T7.4 — Slice close S7** (AC18)

## S5 — Selective storage and runtime services

- [ ] **T5.1 — Dependency-aware emission in `write_control_group`** (R29, R30)
  - Files: `cobolt-codegen/src/lib.rs`.
  - Do: omit a group only when T0.1 and the generator's own scan of authored code prove it unused; retain on any dynamic reference.
  - Verify: AC15 cases — whole-group MOVE, referenced PIC fields, GLOBAL, REDEFINES, BY REFERENCE, Common Code, dynamic references, bindings, debugger watches.
- [ ] **T5.2 — Runtime services replace generator plumbing** (R30, R31)
  - Verify: old helper paragraph names still reachable; event/procedure regression suites and unmodified example forms pass (AC16).
- [ ] **T5.3 — Slice close S5** (AC15, AC16)

## Cross-cutting

- [ ] **T-H — Host sweep (run in every slice close)** (R23, R25)
  - `rcrun run-form`, embedded child forms, `run_form_app` in `cobolt-compiler`, `cobolt-wasm`; unsupported-on-Web → build-time diagnostic plus a `WASM-PARITY` row.
  - Verify: `crates/cobolt-wasm/tests/corpus.rs`, differential tests (`same_output`); Rust 1.92 build; `Cargo.lock` diff justified (AC12).
- [ ] **T-IDE — Editor support and source maps** (R24)
  - Files: `cobolt-ide/src/panels/editor.rs`, `beautify.rs`, `cobolt-dap`, `i18n.rs`, `ui_text_data.rs`.
  - Verify: highlight, completion, hover, diagnostics with spans; stepping into a function or interpolation stops at the original site; source map rebuilt after a line change; `cargo test -p cobolt-ide --bin cobolt-ide i18n` (no empty translation) (AC21).
- [ ] **T-GEN — Generator goldens** (R20–R22)
  - Files: `cobolt-codegen/tests`, `UPDATE_GOLDEN=1` only after reading the diff.
  - Verify: goldens hold the high-level syntax, no legacy expansion, banner kept, authored code verbatim, regeneration identical, zero new diagnostics (AC9); PowerAnalytics workloads compared original vs updated (AC10).
- [ ] **T-KB — System KB per slice** (R27) — `cobolt-compiler` doc tables, `cargo run -p cobolt-ide --example build_chunked_kb`; `prebuilt_chunked_kb_matches_the_published_documentation` green (AC14).
- [ ] **T-DOC — Guide text kept in specs** (R27, §8) — `specs/109-…/guide-notes.md` per slice (COBOL and prose, never Rust; mermaid, screenshot placeholders, Notes, Caveats). On 2.x.x nothing under `README.md` or `docs/`; written into `docs/developers-guide-en.md` (and its five translations deleted) when 2.x.x merges to main (AC14).
- [ ] **T-M — Measurement** (R36, AC11, AC20)
  - Report total/nonblank/executable/generated-only/user-code LOC and bytes per fixture and per construct against the frozen baseline; honest gap report against the 4,344-line study target.
  - Verify: numbers produced by the run; no comment or whitespace stripping counts.
- [ ] **T-FIN — Finalize** — full `cargo test --no-fail-fast` sweep read line by line; NIST census plus finished modules; quarantine sweep of non-Rust files; AC23 scan; confirm no commit/push, merge or forum post unless the operator asks.

## Coverage

AC1 T1.3 · AC2 T2.3, T2.5 · AC3 T2.4 · AC4 T2.1 · AC5 T3.2–T3.4 · AC6 T3.5 · AC7 T4.2 · AC8 T4.3–T4.4 · AC9 T-GEN · AC10 T-GEN · AC11 T-M · AC12 T-H · AC13 T8.2 · AC14 T-KB, T-DOC · AC15 T5.1 · AC16 T5.2 · AC17 T6.2 · AC18 T7.2 · AC19 T8.2 · AC20 T-M, T-GEN · AC21 T-IDE · AC22 T3.3 · AC23 T3.6, T-FIN.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, the Guide text is ready
for the 2.x.x release, and the change is split into feature commits (one per slice)
per the operator's rules. Do **not** commit or push unless the operator asks.
