<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — EXEC RUST compiled into the program binary

- **Status:** draft → approved
- **Folder:** specs/041-exec-rust-native-compilation/
- **Author:** Emerson Lopes (design) · drafted with Claude Opus 5   **Date:** 2026-08-04

## 1. Overview

`EXEC RUST … END-EXEC` parses, resolves its COBOL data references, and reaches the
runtime today — but the executor is a placeholder. It interprets a four-form
micro-language (compound assignment, simple assignment, `let` as a **no-op**, and
integer arithmetic), and **anything it does not recognise is written to a `debug!`
log and skipped** ([`exec_rust.rs:100-131`](../../crates/cobolt-runtime/src/exec_rust.rs)).
A block containing real Rust therefore *succeeds* while doing nothing at all — no
error, no diagnostic, no output. That silence is the defect this spec exists to
remove, as much as the missing capability.

This feature replaces the interpreter with **real Rust, compiled by the developer's
own `rustc` during COBOL compilation and linked into the program's own binary**.
The end user needs no Rust toolchain, exactly as `rcrun` ships as a binary in the
application bundle today. Only data items declared `USAGE OBJECT REFERENCE
RUST-<type>` may cross into a block; they bind as real, typed Rust references in a
generated preamble, which is what makes all 48 `CLASS RUST-*` types usable at once
— a compiled block uses the genuine `std` type rather than one of the 9
hand-written shims in `rust_bridge.rs`.

## 2. Goals / Non-goals

**Goals**

- Real Rust inside `EXEC RUST`: closures, generics, `impl`/traits, pattern
  matching, `?`, iterators, the borrow checker, and all of `std`.
- All 48 `CLASS RUST-*` types (`crates/cobolt-forms/src/model.rs:4419`) usable as
  bound data items, not just the 9 currently bridged — and **developer-defined
  Rust types too**, declared by an item-level block and named by a `CLASS`, making
  the 48 a floor rather than a ceiling.
- `EXEC RUST` usable anywhere COBOL is authored — hand-written sources, event
  handler bodies, lifecycle handlers, user procedures — and, as an item-level
  block, outside the `PROCEDURE DIVISION` to define types.
- **No Rust toolchain on the end user's machine.** Compilation happens on the
  developer's machine during `build_project`; the result ships inside the program
  binary.
- A block that does not compile is a **hard COBOL compile error**, with `rustc`
  diagnostics mapped back to the developer's COBOL line/column.
- A Rust panic is contained and surfaces as a COBOL exception catchable with the
  existing `TRY … CATCH EXCEPTION … END-TRY`
  ([`stmt.rs:783`](../../crates/cobolt-ast/src/stmt.rs)).
- All `EXEC RUST` blocks in one program share a single global Rust context.

**Non-goals**

- **External crates.** v1 is `std`-only; a manifest/vendoring story is deferred.
- **Interpreter fallback.** No dual semantics: a program either compiles its Rust
  or fails to build. The current interpreter is removed, not retained.
- **Runtime dynamic loading.** No `dlopen`/`LoadLibrary`/`cdylib` sidecar; the
  generated code is linked in. (No dynamic loading exists in the workspace today
  and this feature must not introduce it.)
- Passing `PIC` items into a block. Plumbing between `PIC` data and Rust objects
  stays in ordinary COBOL via `INVOKE`/`::`, outside the block.
- Making `EXEC RUST` available in the tree-walking interpreter path used by the
  IDE's *Run* action, if that path does not compile the project first (see Q3).

## 3. User stories

- As a COBOL developer, I want to drop a block of ordinary Rust into a paragraph
  and have it run, so that I can use `std` algorithms and data structures without
  learning the host build system.
- As a COBOL developer, I want a block that does not compile to fail the build
  with a message pointing at my line, so that I never ship a block that silently
  does nothing.
- As a COBOL developer, I want a Rust panic to arrive as a normal COBOL exception,
  so that `TRY … CATCH` remains the one error-handling construct I need.
- As someone shipping an application, I want the compiled result inside my binary,
  so that my users install nothing.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall compile every `EXEC RUST` block in a
  program during `cobolt-compiler::build_project`, using the `rustc` toolchain on
  the building machine.
- **R2 (ubiquitous):** The system shall link the compiled result into the program's
  own output binary, and shall not emit or load a separate shared library.
- **R3 (constraint):** The produced binary shall not require a Rust toolchain, a
  compiler, or any `EXEC RUST` source on the machine that runs it.
- **R4 (ubiquitous):** The system shall bind, as typed Rust references in the
  generated preamble, exactly those data items declared `USAGE OBJECT REFERENCE
  RUST-<type>` and listed in `Stmt::ExecRust::referenced_data`.
- **R5 (constraint):** The system shall reject, at compile time, an `EXEC RUST`
  block that references a COBOL data item which is not a
  `USAGE OBJECT REFERENCE RUST-<type>` item.
- **R6 (constraint):** The system shall require the COBOL name of every bound item
  to be a valid Rust identifier in `snake_case`, and shall reject a bound name that
  is not, naming the offending item.
- **R7 (ubiquitous):** The system shall make all 48 `CLASS RUST-*` types declared
  in the `REPOSITORY` usable as the type of a bound item.
- **R8 (event):** When a `REPOSITORY` `CLASS` names a `Rust.*` type that is neither
  a real `std` type **nor a type defined by an item-level block (R19)**, the system
  shall fail the build with a diagnostic naming that class. The 48 shipped classes
  are therefore a floor, not a ceiling: a developer extends the set by defining a
  type and declaring a `CLASS` for it.
- **R9 (state):** While a program runs, all `EXEC RUST` blocks in it shall share
  one global Rust context, so that state established by an earlier block is visible
  to a later one.
- **R10 (event):** When `rustc` reports an error for a generated block, the system
  shall fail the COBOL build and map each diagnostic to the line and column of the
  developer's own `EXEC RUST` source, using the `Span` already carried on
  `Stmt::ExecRust`.
- **R11 (constraint):** The system shall not silently ignore any part of an
  `EXEC RUST` block. Every statement is compiled or the build fails.
- **R12 (event):** When Rust code in a block panics, the system shall contain the
  panic with `catch_unwind` at the call site and raise a Rust exception catchable
  by `TRY … CATCH RUST-EXCEPTION <name> … END-TRY` (R23). *(No `extern "C"`
  boundary is involved: generated blocks are compiled into the same binary as the
  runtime, so the call is ordinary Rust — see plan §4.)*
- **R13 (constraint):** The system shall not allow a panic to escape the block
  call.
- **R14 (event):** When the building machine has no usable `rustc`, the system
  shall fail the build with an explicit diagnostic naming the missing toolchain and
  shall not fall back to interpretation.
- **R15 (optional):** Where a block's source and binding signature are unchanged
  since the previous build, the system shall reuse the cached compiled artefact
  rather than invoking `rustc` again.
- **R16 (constraint):** v1 shall accept only `std`; a block whose Rust references
  an external crate shall fail the build with a diagnostic saying external crates
  are not yet supported.
- **R17 (constraint):** The system shall build for the **host triple only** and
  shall not cross-compile. An application for a given operating system is built on
  that operating system: macOS → macOS, Linux → Ubuntu/Debian, Windows →
  Windows 10 or later.
- **R18 (event):** When a build is requested for a target other than the host, the
  system shall fail with a diagnostic stating that the application must be built on
  the target operating system.

### Item-level blocks (Rust type definitions)

A statement-level block compiles to a *function body*, where a `struct`, `impl`,
`trait` or `use` cannot be declared in a way other blocks can see. Real Rust needs
**item scope**, so `EXEC RUST` exists in two kinds, mirroring Rust's own
item/statement split.

- **R19 (ubiquitous):** The system shall accept an **item-level** `EXEC RUST` block
  in the `CONFIGURATION SECTION`, following `REPOSITORY`, containing only Rust
  *items* — `use`, `struct`, `enum`, `impl`, `trait`, `fn`, `const`, `type`, `mod`.
  *(Placement co-locates it with the `CLASS RUST-*` declarations, which are the
  other half of the same subject.)*
- **R20 (ubiquitous):** The system shall emit every item-level block at module
  scope of the generated Rust, so that types, traits and functions it defines are
  visible to every statement-level block in the program.
- **R21 (constraint):** The system shall reject an item-level block that contains
  statements, and a statement-level block that appears outside the
  `PROCEDURE DIVISION`, each with a diagnostic naming the block's location.
- **R22 (ubiquitous):** The system shall allow a data item to be declared
  `USAGE OBJECT REFERENCE` against a `CLASS` naming a type defined by an
  item-level block, and shall bind it in a statement-level block exactly as a
  `std` type is bound (R4).

### Rust exceptions

- **R23 (event):** When a contained panic (R12) occurs inside a `TRY` body, the
  system shall bind its message and details, in plain text, to the name given by a
  `CATCH RUST-EXCEPTION <name>` clause and run that clause's body, so that
  `DISPLAY <name>` prints the failure without the developer inspecting the text.
- **R24 (constraint):** A plain `CATCH EXCEPTION` clause shall **not** catch a Rust
  panic. A panic is a distinct failure class, and folding it into the general
  handler would let a memory-safety or logic fault be reported as a business error.
  A `TRY` may carry both clauses; each catches only its own class.
- **R25 (state):** While a `TRY` body has no `CATCH RUST-EXCEPTION` clause, a
  contained panic shall propagate after `FINALLY` runs, terminating the program
  with the panic's plain-text message.

## 5. Acceptance criteria

- [ ] **AC1** — A program whose block calls a real `std` API (e.g.
      `user_name.push_str("x"); let n = user_name.chars().rev().count();`) builds,
      runs, and the mutation is visible to COBOL afterwards. *(R1, R4, R9)*
- [ ] **AC2** — A block using a closure, a generic, an iterator chain, `match` and
      `?` compiles and runs. *(R1)* — proves the interpreter's limits are gone.
- [ ] **AC3** — The built binary runs on a machine with no Rust toolchain
      installed, verified by building in one environment and executing where
      `rustc` is absent. *(R2, R3)*
- [ ] **AC4** — A block with a deliberate Rust type error fails the build, and the
      reported line/column point at the developer's `EXEC RUST` source, not at
      generated code. *(R10)*
- [ ] **AC5** — A block containing a statement the old interpreter would have
      skipped (e.g. `foo();`) fails the build rather than succeeding silently.
      *(R11)* — this is the regression guard for the reported defect.
- [ ] **AC6** — A bound item that is a `PIC` item is rejected at compile time with
      a message naming it. *(R5)*
- [ ] **AC7** — A bound item named `WS-USER-NAME` is rejected with a message
      telling the developer the Rust-side name must be `snake_case`. *(R6)*
- [ ] **AC8** — Each of the 48 `CLASS RUST-*` types is exercised by at least one
      test declaring an item of that type and using it inside a block. *(R7)*
- [ ] **AC9** — `CLASS RUST-NOPE IS "Rust.Nope"` fails the build naming the class.
      *(R8)*
- [ ] **AC10** — Two blocks in one program share state: the first stores into a
      bound `Rust.Vec`, the second reads it back. *(R9)*
- [ ] **AC11** — A block that panics (e.g. an out-of-range index) is caught by
      `TRY … CATCH RUST-EXCEPTION e … END-TRY`, the program continues, and
      `DISPLAY e` prints the panic message and details in plain text with no
      substring inspection. *(R12, R13, R23)*
- [ ] **AC17** — The same panic inside a `TRY` whose only clause is
      `CATCH EXCEPTION e` is **not** caught; it propagates after `FINALLY` runs.
      *(R24, R25)*
- [ ] **AC18** — A `TRY` carrying both `CATCH EXCEPTION` and
      `CATCH RUST-EXCEPTION` routes a COBOL exception to the first and a panic to
      the second. *(R24)*
- [ ] **AC19** — An item-level block defining `struct Point { x: i64, y: i64 }` with
      an `impl`, plus `CLASS MY-POINT IS "Rust.Point"`, allows
      `01 origin USAGE OBJECT REFERENCE MY-POINT` to be bound and its methods called
      from a statement-level block. *(R19, R20, R22, revised R8)*
- [ ] **AC20** — A type defined in one item-level block is usable from a
      statement-level block in a *different* paragraph and from a form event
      handler. *(R20)*
- [ ] **AC21** — An item-level block containing a statement is rejected, and a
      statement-level block placed outside `PROCEDURE DIVISION` is rejected, each
      naming the location. *(R21)*
- [ ] **AC12** — With `rustc` unavailable, the build fails with a diagnostic naming
      the missing toolchain; no program is produced. *(R14)*
- [ ] **AC13** — An unchanged block does not re-invoke `rustc` on a second build,
      demonstrated by a measured build-time or invocation-count assertion. *(R15)*
- [ ] **AC14** — A block with `use serde::Serialize;` fails with the
      "external crates not yet supported" diagnostic. *(R16)*
- [ ] **AC15** — A build on each supported host produces a working binary for that
      host: macOS → macOS, Linux → Ubuntu/Debian, Windows → Windows 10+. *(R17)*
- [ ] **AC16** — Requesting a non-host target fails with a diagnostic telling the
      developer to build on the target operating system. *(R18)*

## 6. Constraints & steering check

- **i18n (6 languages):** Required for any user-facing IDE string — build-failure
  messages surfaced in the IDE Output panel, and the missing-toolchain diagnostic.
  Compiler diagnostics themselves follow existing diagnostic conventions. COBOL
  identifiers and generated Rust stay English (`product.md`).
- **Generated-code / regenerate contract:** The generated Rust is a **build
  artefact**, in the same sense as generated `.cbl`. It must never be hand-edited,
  must be reproducible from source on every build, and should carry a generated-by
  banner consistent with `cobolt-codegen::write_header`.
- **Docs (English guide):** `docs/developers-guide-en.md` needs a new section — what
  `EXEC RUST` now is, the `USAGE OBJECT REFERENCE` restriction, the `snake_case`
  rule, shared context, `std`-only, and the toolchain requirement for *building*.
  Translations are user-maintained and must not be edited.
- **System KB:** This changes runtime/compiler behaviour, so the `cobolt-compiler`
  KB documentation tables must be updated in the same change. Note the operator
  lifted the chunked-reindex suspension on 2026-07-31, which `tech.md` still
  records as suspended (2026-07-29) — see Q5.
- **Fix vs feature:** **Feature.** New functionality, so it belongs on the
  `features` branch. Version bump is deliberately *not* asserted here — see Q4.
- **Tests:** Every acceptance criterion above is verifiable and quantified.
  AC3 and AC13 require measured evidence, not assertion.

## 7. Open questions

- ~~**Q1: which target triples must be supported?**~~ **RESOLVED 2026-08-04
  (operator):** No cross-compilation. Host triple only — an application is built on
  the operating system it targets (macOS → macOS, Linux → Ubuntu/Debian, Windows →
  Windows 10+). Recorded as R17/R18; the build pipeline needs no cross-linkers.
- ~~**Q2:** how is the shared global context scoped?~~ **RESOLVED in plan §4:**
  per process / one run-unit, since the built binary is one process running one
  `Interpreter`. Consequence to document: `CANCEL` does not reset it in v1, and a
  second `CALL` of the same program sees prior state.
- ~~**Q3:** does *Run* require a build?~~ **RESOLVED in plan §4:** a program
  containing `EXEC RUST` is built before it runs, and the IDE's *Run* performs that
  build automatically — one meaning for `EXEC RUST`, no dual semantics. See **Q8**
  for the cost this now carries.
- **Q4:** `tech.md` says features bump the **minor**; the operator's standing rule
  is that only they raise `x` or `y`. Assumed here: leave the version to the
  operator and do not bump the minor unprompted. Confirm.
- **Q5:** `tech.md` records the chunked-KB reindex as suspended (2026-07-29); the
  operator lifted that on 2026-07-31. Should `tech.md` be corrected as part of this
  work, and is `build_chunked_kb` expected to run for this change?
- ~~**Q6:** exception type/name for a contained panic?~~ **RESOLVED 2026-08-04
  (operator):** a dedicated `CATCH RUST-EXCEPTION <name>` clause, not a message
  prefix — `DISPLAY <name>` prints the message and details in plain text and the
  developer never inspects substrings. Recorded as R23–R25. *(Investigation note:
  `TRY/CATCH` today catches only `RuntimeError::UserException` and binds its
  message string — `interpreter.rs:2334` — so there is no exception-class
  mechanism to reuse; R23 adds one for this class.)*
- ~~**Q7:** is `EXEC RUST` permitted in a form's event handler?~~ **RESOLVED
  2026-08-04 (operator):** yes — and anywhere COBOL procedure code is authored:
  hand-written sources, event handler bodies, form-level lifecycle handlers, and
  user/common procedures. **No regenerate-contract breach:** a handler's body lives
  in `EventBinding.code` in the `.cfrm` and codegen emits it into the generated
  `.cbl`, which is never authored by hand. Additionally, `EXEC RUST` may appear
  *outside* the `PROCEDURE DIVISION` as an item-level block (R19–R22).
- ~~**Q8:** accept a cargo build on every *Run* of a form containing a block?~~
  **RESOLVED 2026-08-04 (operator): accepted.** Blocks stay allowed in event
  handlers, and a program containing `EXEC RUST` is built before it runs. The cost
  is **compile-time only** — execution is unchanged, since the built binary
  interprets the COBOL exactly as now while the Rust blocks become native code
  instead of the removed micro-interpreter. Cold builds (the `eframe` + runtime
  dependency tree) are the slow case; warm rebuilds recompile only the generated
  crate. Build progress must be reported in the Output panel so the pause is
  explained rather than mysterious.
