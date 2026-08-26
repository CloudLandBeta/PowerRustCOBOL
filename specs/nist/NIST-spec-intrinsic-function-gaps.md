# NIST-spec — intrinsic function gaps

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **IF module, 45 programs, 21 clean today**. 11 programs
  use the `(ALL)` table-argument form.

## 1. Overview

`docs/cobol85-supported-syntax-en.md` states that "the **complete COBOL-85
standard intrinsic set** is implemented", listing 50-odd functions. The
implementation is genuinely broad — the IF module's 21 clean programs bear that
out. Three specific gaps remain.

### 1a. Fractional arguments do not lex

Every IF program tests its function with fractional constants written with a
leading decimal point: `FUNCTION ACOS(.999)`, `FUNCTION ANNUITY(.09, A)`. This
is **not an intrinsic-function defect** — it is
`NIST-spec-numeric-literals.md`, and it accounts for 37 of the 45 IF programs'
first errors. It is listed here only so the IF module's score is not
misattributed. **No requirement in this spec covers it.**

### 1b. The `(ALL)` table argument

COBOL-85 lets a table be passed whole to the statistical functions by
subscripting it with the reserved word `ALL`:

```cobol
COMPUTE WS-NUM = FUNCTION MAX(IND(ALL)).
COMPUTE WS-NUM = FUNCTION MEAN(IND(ALL)).
COMPUTE WS-NUM = FUNCTION MEDIAN(IND(ALL)).
COMPUTE WS-NUM = FUNCTION MIDRANGE(IND(ALL)).
```

Measured:

```
L22   expected literal after ALL          |  COMPUTE WS-NUM = FUNCTION MAX(IND(ALL)).
L22   expected expression, found RParen
```

`ALL` is being read as the figurative-constant prefix (`ALL "X"`), so the parser
demands a literal after it. In a subscript position `ALL` means "every
occurrence", expanding to as many arguments as the table has elements.

Programs affected: IF119A (MAX), IF120A (MEAN), IF121A (MEDIAN), IF122A
(MIDRANGE) and 7 more — the functions that take a variable-length argument list:
`MAX`, `MIN`, `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`,
`STANDARD-DEVIATION`, `ORD-MAX`, `ORD-MIN`.

### 1c. Unrecognised function names return 0

The supported-syntax doc records this as avoid-list item 5:

> Unrecognised intrinsic‑function names still return **0**.

A NIST program calling a function we do not implement would silently compute
zero and report `FAIL` with no indication why. For a conformance suite that is
the worst possible failure mode.

## 2. Goals / Non-goals

- **Goals:** the `(ALL)` argument form; a real diagnostic for unknown function
  names; confirmation that the implemented set matches what the IF module calls.
- **Non-goals:**
  - Post-1985 intrinsics.
  - Changing any implemented function's result. If an IF program disagrees with
    our result, that is a separate finding, not this spec.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall accept `ALL` as a subscript in a
  reference used as an intrinsic-function argument, meaning every occurrence of
  that table.
- **R2 (ubiquitous):** `ALL` shall be usable in one dimension of a
  multi-dimensional table with ordinary subscripts in the others, expanding in
  row-major order.
- **R3 (state):** While the table has an `OCCURS … DEPENDING ON` clause, `ALL`
  shall expand to the current number of occurrences, evaluated when the function
  is called.
- **R4 (constraint):** `ALL` as a figurative-constant prefix (`MOVE ALL "X" TO
  Y`) shall be unaffected.
- **R5 (event):** When a program calls a `FUNCTION` name the system does not
  implement, the system shall report it as an error at compile time rather than
  returning 0 at run time.
- **R6 (ubiquitous):** The system shall verify at implementation time that every
  function name the IF module calls is implemented, and shall list any that are
  not.

## 4. Acceptance criteria

- [ ] AC1 — `FUNCTION MAX(IND(ALL))` over a 5-element table returns the largest
      element, and `FUNCTION MEAN(IND(ALL))` their mean.
- [ ] AC2 — `FUNCTION SUM(TBL(ALL, 2))` sums one column of a 2-D table (R2).
- [ ] AC3 — `ALL` over an `OCCURS DEPENDING ON` table honours the current count
      (R3).
- [ ] AC4 — `MOVE ALL "X" TO Y` is unchanged (regression test).
- [ ] AC5 — `FUNCTION NO-SUCH-THING(1)` is a compile error naming the function;
      avoid-list item 5 is removed from
      `docs/cobol85-supported-syntax-en.md`.
- [ ] AC6 — R6's audit is recorded in this file, listing any missing function.
- [ ] AC7 — The IF module rises from 21 / 45 once
      `NIST-spec-numeric-literals.md` has landed too, scored on each program's
      own `PASS`/`FAIL` report.

## 5. Constraints & steering check

- **i18n:** none — COBOL function names stay English in every UI language
  (CLAUDE.md CRITICAL constraint).
- **Generated-code contract:** the RAD generator emits `FUNCTION` calls for some
  bindings; R5 turns a silent 0 into a compile error, so any generated call to
  an unimplemented function would start failing loudly. That is the point, but
  `/plan` must grep the generator for function names first.
- **System KB:** intrinsic functions are documented in the `cobolt-compiler` doc
  constants. If R5 or R6 changes the documented set, `chunked.data` must be
  rebuilt in the same change (CLAUDE.md, System KB rule).
- **Fix vs feature:** **fix**.

## 6. Open questions

- Q1: R5 is a behaviour change — programs that today silently get 0 would stop
  compiling. That is correct, but it could break existing user code that has an
  unnoticed typo. Recommendation: error, with the message naming the closest
  implemented function. **Operator ruling wanted** on error vs warning.
- Q2: Does `ALL` belong in the lexer (a distinct subscript token) or the parser
  (context-sensitive)? Recommendation: parser — `ALL` must keep its
  figurative-constant meaning elsewhere (R4).
