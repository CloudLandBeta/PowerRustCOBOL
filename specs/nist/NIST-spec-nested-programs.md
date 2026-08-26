# NIST-spec — nested programs, END PROGRAM, GLOBAL / EXTERNAL / COMMON

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **IC module, 47 programs, 32 clean today**; plus OBIC
  (3). Measured: 13 programs use `END PROGRAM`, 5 use `EXTERNAL`, 4 use
  `GLOBAL`.

## 1. Overview

COBOL-85 introduced the **nested program**: a program contained inside another,
delimited by `END PROGRAM name.`, with name scoping rules that go with it.

PowerRustCOBOL has partial support — `END PROGRAM` appears in the lexer, the
parser and the AST — and the IC module already scores 32 / 47, the best in-scope
ratio after RL. What is missing is the scoping semantics, and the harness census
shows the shape of the gap: **56 programs report `paragraph '…' is not a
paragraph or section of this program`**, naming paragraphs like
`SEQ-TEST-06-END`, `DE-LETE-1`, `FAIL-1`, `PASS-1`, `PRINT-DETAIL-1`, `R1-EXIT`,
`RD-1`, `RET-2`.

The diagnostic itself is written correctly and says exactly the right thing:

> PERFORM and GO TO reach only procedures declared in the same program; a
> paragraph of that name elsewhere in the compilation unit is not visible here.

That is the correct COBOL-85 rule. The question this spec must settle is whether
those 56 programs are genuinely referencing out-of-scope paragraphs — in which
case NIST expects the error and the harness must **not** count it as a failure —
or whether the compilation unit is being split into programs incorrectly, so
paragraphs that *are* in the same program appear not to be.

**That question is unresolved and is Q1.** It must be answered before any code
changes, because the two answers lead to opposite work.

## 2. Goals / Non-goals

- **Goals:** correct COBOL-85 name scoping across nested programs, and the
  `GLOBAL`, `EXTERNAL`, `COMMON` and `INITIAL` attributes.
- **Non-goals:**
  - Separately-compiled program linking beyond the existing `CALL` mechanism.
  - Changing the diagnostic quoted above — it is right.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall parse a compilation unit containing
  nested programs delimited by `END PROGRAM name.`, to arbitrary nesting depth.
- **R2 (ubiquitous):** The system shall parse `PROGRAM-ID name [IS] [COMMON]
  [INITIAL] [PROGRAM].`
- **R3 (state):** While a contained program is `COMMON`, it shall be callable by
  its siblings and their descendants, not only by its immediate parent.
- **R4 (event):** When a program is `INITIAL`, the system shall restore its
  WORKING-STORAGE to its `VALUE`-declared initial state on every entry.
- **R5 (ubiquitous):** A data item declared `GLOBAL` shall be visible to
  contained programs; one that is not shall not be.
- **R6 (ubiquitous):** A data item declared `EXTERNAL` shall denote one storage
  area shared by every program in the run unit that declares it with the same
  name.
- **R7 (ubiquitous):** A file declared `GLOBAL` or `EXTERNAL` shall follow the
  same visibility and sharing rules as data.
- **R8 (constraint):** Procedure names shall remain local to their own program,
  as the existing diagnostic states. This rule is **not** relaxed.
- **R9 (ubiquitous):** `PROCEDURE DIVISION USING` parameter lists shall accept
  separator commas (`NIST-spec-separators.md` R3) — measured as the first error
  in IC201A, IC203A and IC207A.

## 4. Acceptance criteria

- [ ] AC1 — Q1 is answered, in writing, before implementation starts.
- [ ] AC2 — A compilation unit with three levels of nesting parses, and each
      program's paragraphs are visible only within it.
- [ ] AC3 — A `COMMON` contained program is callable by a sibling; a non-`COMMON`
      one is not, and the attempt is diagnosed.
- [ ] AC4 — An `INITIAL` program's WORKING-STORAGE is re-initialised on the
      second `CALL`; a non-`INITIAL` program's is not.
- [ ] AC5 — A `GLOBAL` item is readable and writable from a contained program;
      a non-`GLOBAL` item of the same name is not visible.
- [ ] AC6 — Two programs declaring the same `EXTERNAL` item see each other's
      writes.
- [ ] AC7 — IC201A, IC203A and IC207A parse (R9).
- [ ] AC8 — The IC module rises from 32 / 47, scored on each program's own
      `PASS`/`FAIL` report.
- [ ] AC9 — The existing `CALL` / `CANCEL` behaviour and the `cobolt-runtime`
      test suite are unaffected.

## 5. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator emits a single program per
  form; nesting is additive and generated code is unaffected. AC9 pins it.
- **Docs:** the Developer's Guide gains a nested-program section. This is a real
  gap for the target audience: isCOBOL and PowerCOBOL developers use contained
  programs, and `GLOBAL` is how they share working storage.
- **Fix vs feature:** **fix**.

## 6. Open questions

- Q1 — **blocking.** Are the 56 `is not a paragraph or section of this program`
  reports correct COBOL-85 behaviour that NIST expects, or an artefact of how a
  multi-program member is split? Method: take SQ102A (3 reports) and IC-module
  members with `SUBPRG` headers, and check by hand whether the named paragraph
  is declared in the same program. The harness's `dump` mode plus the member
  splitter give this directly.
- Q2: Does `EXTERNAL` storage live for the run unit or the process? Run unit,
  consistent with the project's existing single-run-unit locking model.
- Q3: `INITIAL` requires a snapshot of declared `VALUE`s. Does
  `CobolEnvironment` already retain them after initialisation, or must the
  initial image be kept separately?
