# NIST-spec — numeric literals with a leading decimal point

- **Status:** ✅ **IMPLEMENTED 2026-08-25** (version 1.62.10)
- **Result:** NIST in-scope PASS **222 → 237 / 434** (51.2 % → 54.6 %).
  Intrinsic functions 21 → 29, Nucleus 25 → 29, Sort/Merge 27 → 30.
  The `expected expression, found …` root-cause bucket fell **36 → 10**, and
  those 10 are a *different* construct sharing the message (`SET SW-1 TO ON`,
  `SET A, B, C TO 1`), owned by
  [`NIST-spec-special-names.md`](NIST-spec-special-names.md) and
  [`NIST-spec-separators.md`](NIST-spec-separators.md).
- **Plan:** [`NIST-plan-numeric-literals.md`](NIST-plan-numeric-literals.md) ·
  **Tasks:** [`NIST-tasks-numeric-literals.md`](NIST-tasks-numeric-literals.md)
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** **48 of 459 programs (10.5 %)** contain such a literal.
  **37 programs fail on it as their first error** — the second-largest
  root-cause bucket in the baseline, and the reason the IF (intrinsic function)
  module scores only 21 / 45.

## 1. Overview

COBOL-85 allows a numeric literal to begin with a decimal point: `.5` is one
half. The rule is that a numeric literal must contain at least one digit and
must not *end* with a decimal point; a *leading* point is explicitly permitted.

PowerRustCOBOL's lexer requires a digit before the point. `.00001` is read as a
period followed by `00001`, and `00001` is then classified as a **level
number**:

```
L16   expected expression, found Period          |  MOVE    .00001  TO WS-NUM.
L16   expected To, found Period                  |  ...
L16   unexpected token in statement: LevelNumber(1)
```

Inside a `FUNCTION` argument the same literal ends the argument list early:

```
L17   expected expression, found Period          |  COMPUTE WS-NUM = FUNCTION ACOS(.999).
L17   expected RParen, found Period
L17   unexpected token in statement: IntegerLiteral(999)
```

CCVS85 leans on this heavily — the IF module tests every intrinsic function
with fractional arguments written in exactly this form:

| Program | Source |
|---|---|
| IF101A | `COMPUTE WS-NUM = FUNCTION ACOS(.999).` |
| IF102A | `IF (FUNCTION ANNUITY(.09, A) >= MIN-RANGE) AND` |
| IF103A | `COMPUTE WS-NUM = FUNCTION ASIN(.999).` |
| IF104A | `COMPUTE WS-NUM = FUNCTION ATAN(.999).` |
| NC101A | `77  A05ONES-DS-00V05  PICTURE SV9(5) VALUE .11111.` |
| NC101A | `77  A01ONE-DS-P0801   PICTURE SP(8)9 VALUE .000000001.` |
| NC101A | `IF WRK-DU-5V1-1 = .1 PERFORM PASS PERFORM PRINT-DETAIL` |

## 2. Goals / Non-goals

- **Goals:** lex `.5`, `+.5`, `-.5` as numeric literals everywhere a numeric
  literal is valid — `VALUE`, `MOVE`, `COMPUTE`, conditions, `FUNCTION`
  arguments, `WHEN`.
- **Non-goals:**
  - Accepting a literal that *ends* with a decimal point (`5.` is `5` followed
    by a sentence-ending period, and must stay that way).
  - Changing `DECIMAL-POINT IS COMMA` handling, beyond making `,5` work by the
    same rule under that clause.

## 3. The disambiguation problem

A period in COBOL is either a decimal point or the end of a sentence. `.5`
following a space is a literal; `.` following `WS-NUM` and preceded by no digit
is a terminator. The rule that resolves it:

> A period is the start of a numeric literal **only if** it is immediately
> followed by a digit **and** immediately preceded by a space, `(`, `,`, `=`, an
> arithmetic operator, or the start of the line.

The critical exclusion is a period *immediately following a digit or a letter* —
`VALUE 1.` and `MOVE X TO Y.` must keep terminating the sentence. Note also
`PICTURE -9(9).9(9)`: the `.9` there is preceded by `)`, and PICTURE strings are
lexed as their own token anyway.

The ambiguous case `MOVE X TO Y. .5` does not arise in practice and either
reading is acceptable; the standard resolves it by requiring a space after a
sentence-ending period.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall lex a period immediately followed by one
  or more digits as the start of a numeric literal when the period is preceded
  by a space, `(`, `,`, `=`, an arithmetic operator, or the start of a line.
- **R2 (ubiquitous):** The system shall accept an optional leading `+` or `-`
  before such a literal.
- **R3 (constraint):** The system shall continue to treat a period that
  immediately follows a digit, letter, `)` or quotation mark as a sentence
  terminator or a PICTURE character.
- **R4 (state):** While `DECIMAL-POINT IS COMMA` is in effect, R1-R3 shall apply
  to the comma instead of the period.
- **R5 (ubiquitous):** The resulting literal shall carry its exact scale, so
  `.000000001` is scale 9 and not rounded — CCVS85's `A01ONE-DS-P0801` depends
  on it (`PICTURE SP(8)9 VALUE .000000001`).
- **R6 (constraint):** The system shall not classify a digit string that follows
  a decimal point as a level number.

## 5. Acceptance criteria

- [x] AC1 — `MOVE .00001 TO WS-NUM.` parses and stores 0.00001.
- [x] AC2 — `COMPUTE WS-NUM = FUNCTION ACOS(.999).` parses; the argument is
      0.999, not 999.
- [x] AC3 — `77 X PICTURE SV9(5) VALUE .11111.` gives X the value 0.11111.
- [x] AC4 — `IF WRK-DU-5V1-1 = .1` compares against one tenth.
- [x] AC5 — `MOVE X TO Y.` and `77 N PIC 9 VALUE 1.` are unchanged — a
      regression test pins both.
- [x] AC6 — Under `DECIMAL-POINT IS COMMA`, `VALUE ,11111` behaves as AC3.
- [x] AC7 — The `expected expression, found …` root-cause bucket empties, and
      the IF module's clean count rises materially from 21 / 45.
      *(The bucket was 37 programs when this spec was written; after the
      source-format fix landed and the census reordered it is **36**. Re-measure
      before claiming the criterion, do not trust either number.)*
- [x] AC8 — Scale is exact to at least 18 digits (R5), verified by executing a
      program that prints the value rather than by parse success alone.

## 6. Note — the DATA DIVISION may already be misreading these

Measured: `77 A05ONES-DS-00V05 PICTURE SV9(5) VALUE .11111.` produces **no parse
error** today. That is not evidence it is correct — it is evidence the value is
being taken from a mis-lexed token stream without complaint. AC3 and AC8 must
therefore be verified by running the program and reading the value, not by a
clean parse. This is the same trap
`NIST-spec-harness-and-baseline.md` R8 describes.

## 7. Constraints & steering check

- **i18n:** none directly; R4 touches the decimal-comma path, which localized
  numeric display depends on.
- **Generated-code contract:** the RAD generator emits `VALUE ZERO` and integer
  literals, so no impact expected.
- **Docs:** `docs/cobol85-supported-syntax-en.md` — the literals section
  currently says "Literals: integer, decimal, …" without stating the leading
  point is allowed. Correct it.
- **Fix vs feature:** **fix**.

## 8. Open questions — all resolved

- **Q1** (does `parse_decimal_token` handle a missing integer part?) — **moot.**
  The lexer is not involved; see §9.
- **Q2** (R1's preceding-character set) — **replaced.** The implementation uses
  adjacency instead of a character list; see §9.
- **Q3**, raised during `/tasks` (should `MOVE X TO Y.5` warn?) — **resolved by
  the operator, 2026-08-25: follow the standard, and raise an *error*.** No new
  machinery was needed — COBOL-85 has no reading for `5` as a statement, so the
  parser already rejects it. `malformed_period_digits_is_an_error` now pins it,
  guarding against the leading-point rule quietly turning a typo into an extra
  receiver.

## 9. Implementation note — R1 and R3 were satisfied differently

The requirements are met; the mechanism is not the one §3 and R1 describe.

R1 asks the **lexer** to form the literal when the period is preceded by a
space, `(`, `,`, `=` or an operator. That cannot work: a leading dot also starts
a **numeric-edited PICTURE**, and `PIC .9999/99999,99999,99` is
lexically identical to `VALUE .9999`. CCVS85 contains both — 157 leading-dot
`VALUE`s and 9 leading-dot `PICTURE`s (`WRK-NE-1`). A lexer-formed literal would
reach `parse_pic_clause` and rebuild the template as `0.9999`, corrupting the
picture with no diagnostic.

It is done in **`parse_literal_inner`** instead, using **span adjacency**: the
literal forms only when the digits start exactly where the period ends. This
- keeps PICTURE parsing untouched — `parse_pic_clause` never calls the literal
  parser, so R3 holds structurally rather than by rule;
- reuses the pattern already proven here for `DECIMAL-POINT IS COMMA`
  (`expr.rs`), which had solved the mirror-image problem the same way;
- recovers the scale from the **span width**, because the token value has
  already lost the leading zeros (`.00001` arrives as the value `1`) — this is
  what satisfies R5;
- accepts `LevelNumber` as well as `IntegerLiteral` after the point, because a
  period sets the lexer's "at line start" flag, so `.1` arrives as
  `LevelNumber(1)` — this is what satisfies R6, without changing the
  level-number rule that every data description entry depends on.

What replaces R1's character list: **what precedes the period is irrelevant;
only the absence of a gap after it matters.** Simpler, and it cannot be
defeated by an unlisted character.
