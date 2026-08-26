# NIST-spec — statement-level grammar gaps

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** six independent gaps, each small on its own, together
  blocking programs across NC, IF, IX, SQ and OBIC. All six are reproduced in
  probes `p6_stmts.cbl` and `p9_misc.cbl`.

## 1. Overview

These are verb-level forms COBOL-85 defines that PowerRustCOBOL does not accept.
They are grouped in one spec because each is a contained parser change with no
shared design; splitting them into six specs would add ceremony without adding
clarity. `/tasks` should still produce one task per gap.

`docs/cobol85-supported-syntax-en.md` currently claims "The COBOL-85 verb /
clause set is **fully covered**." Each subsection below is a counter-example,
and the doc is corrected as part of this spec (GOLDEN RULE #3).

## 2. Gap 1 — MULTIPLY / DIVIDE format 1 with multiple receivers

COBOL-85 format 1 takes a **list** of receiving operands, each with its own
optional `ROUNDED`:

```
MULTIPLY {identifier-1|literal-1} BY {identifier-2 [ROUNDED]} … [SIZE ERROR …]
```

Today only a single receiver is accepted before `GIVING`. The supported-syntax
doc records `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …]` — multiple
receivers on `GIVING` only.

Measured (NC101A):

```
096900     MULTIPLY WRK-DU-4P1-1 BY WRK-DU-5V1-1 ROUNDED WRK-DU-2P4-1
097000         WRK-DU-6V0-1 ROUNDED WRK-DU-6V0-2 WRK-DU-0V12-1.
```

Probe result: `unexpected token in statement: Identifier("B")` on
`MULTIPLY 2 BY A ROUNDED B ROUNDED C.`

- **R1:** The system shall accept a list of receiving operands on `MULTIPLY …
  BY` and on `DIVIDE … INTO`, each with an independent optional `ROUNDED`.
- **R2:** Each receiver shall be computed from the *original* value of the
  operand, not from a previously updated receiver.

## 3. Gap 2 — PERFORM … TIMES with an identifier

COBOL-85: `PERFORM procedure-name {identifier-1|integer-1} TIMES`. The count may
be a data item.

Measured (NC102A), where `77 THREE PICTURE IS 9 VALUE IS 3`:

```
076400     PERFORM PFM-C THREE TIMES.
```

Probe result: `unexpected token in statement: Identifier("THREE")`.

- **R3:** The system shall accept an identifier as the repeat count of
  `PERFORM … TIMES`.
- **R4:** The count shall be evaluated once, at the start of the `PERFORM`; a
  later change to the item shall not change the number of iterations.

## 4. Gap 3 — EVALUATE with a conditional-expression subject

COBOL-85 allows the `EVALUATE` subject to be a conditional expression, including
a class condition, matched against `WHEN TRUE` / `WHEN FALSE`.

Measured (NC225A):

```
034700     EVALUATE WRK-XN-00001-1 NUMERIC
```

Probe result: `unexpected token in statement: Identifier("NUMERIC")`, followed
by a cascade that swallows the `WHEN` branches and `END-EVALUATE`.

- **R5:** The system shall accept a class condition (`NUMERIC`, `ALPHABETIC`,
  `ALPHABETIC-LOWER`, `ALPHABETIC-UPPER`, and any `CLASS` declared in
  SPECIAL-NAMES) as an `EVALUATE` subject.
- **R6:** The system shall accept sign conditions (`POSITIVE`, `NEGATIVE`,
  `ZERO`) and condition-names in the same position.
- **R7:** Such a subject shall be matched against `WHEN TRUE` and `WHEN FALSE`.

## 5. Gap 4 — WHEN with a signed literal and THRU

Measured (IF116A, IF133A, IF135A and 8 more):

```
037500     WHEN -0.000020 THRU 0.000020
```

Probe result:

```
L19   expected comparison operator in condition
L19   unexpected token in statement: Through
L19   unexpected token in statement: DecimalLiteral { mantissa: 20, scale: 6 }
```

The leading `-` makes the parser read the `WHEN` object as the start of a
condition rather than as a signed literal range.

- **R8:** The system shall accept a signed numeric literal as a `WHEN` object.
- **R9:** The system shall accept `THRU`/`THROUGH` ranges whose bounds are
  signed numeric literals.

## 6. Gap 5 — CLOSE with the reel/unit and lock phrases

COBOL-85: `CLOSE file [{REEL|UNIT} [FOR REMOVAL]] [WITH {NO REWIND|LOCK}]`.

Measured in 6 programs (IX204A, IX401M, SQ211A, SQ215A and peers):

```
072700     CLOSE    IX-FD2      WITH LOCK.
```

Probe result: `unexpected token in statement: With`.

- **R10:** The system shall parse `CLOSE … WITH LOCK`, `CLOSE … WITH NO REWIND`,
  `CLOSE … REEL`, `CLOSE … UNIT` and `CLOSE … FOR REMOVAL`.
- **R11:** `WITH LOCK` shall prevent the file being reopened in the same run
  unit. The reel/unit phrases apply to multi-volume tape and shall parse and be
  accepted as no-ops on disk files, consistent with the existing single-run-unit
  model recorded in the supported-syntax avoid-list.

## 7. Gap 6 — qualified and multi-subscripted operands in SET

Measured (NC248A):

```
034900     SET INDEX1 TO TABLE2-REC OF TABLE2 (INDEX2).
```

Probe result: `unexpected token in statement: LParen`. The same shape in `MOVE`
(`CELL OF COLS OF ROWS (IDX-A IDX-B)`) gives
`expected RParen, found Identifier("IDX-B")`.

- **R12:** The system shall accept an operand that combines `OF`/`IN`
  qualification with a subscript list in every statement that takes an
  identifier, `SET` included.
- **R13:** Subscript lists in that position shall follow
  `NIST-spec-separators.md` R4 (space-separated is legal).

## 8. Gap 7 — INSPECT … CONVERTING

Used by 2 programs. COBOL-85:
`INSPECT id CONVERTING chars-1 TO chars-2 [BEFORE|AFTER INITIAL …]`.

- **R14:** The system shall parse and execute `INSPECT … CONVERTING`, honouring
  the `BEFORE`/`AFTER INITIAL` region already modelled by `InspectRegion`.

## 9. Acceptance criteria

- [ ] AC1 — `MULTIPLY 2 BY A ROUNDED B ROUNDED C.` parses; A, B and C each get
      twice their own original value.
- [ ] AC2 — NC101A's two-line `MULTIPLY` with five receivers parses and computes
      correctly.
- [ ] AC3 — `PERFORM PFM-C THREE TIMES.` runs the paragraph three times;
      changing `THREE` inside the loop does not change the count.
- [ ] AC4 — `EVALUATE X NUMERIC … WHEN TRUE …` selects the right branch for a
      numeric and a non-numeric value.
- [ ] AC5 — `WHEN -0.000020 THRU 0.000020` matches −0.00001 and does not match
      −0.0001.
- [ ] AC6 — `CLOSE F WITH LOCK.` parses, and reopening F in the same run unit
      fails with the standard file status.
- [ ] AC7 — `SET INDEX1 TO TABLE2-REC OF TABLE2 (INDEX2).` parses and sets the
      index.
- [ ] AC8 — `INSPECT … CONVERTING` converts, including under `AFTER INITIAL`.
- [ ] AC9 — NC101A, NC102A, NC225A, NC248A, IF116A, IF133A, IF135A, IX204A,
      IX401M, SQ211A and SQ215A lose these diagnostics in the harness census.
- [ ] AC10 — `docs/cobol85-supported-syntax-en.md` no longer claims full
      COBOL-85 verb coverage, and each form above is listed accurately.

## 10. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator emits `MOVE`, `PERFORM`,
  `EVALUATE` and file verbs; every requirement here is additive, so generated
  code is unaffected. The forms suite must stay green.
- **System KB:** none of these change a control, property, method or event, so
  `chunked.data` does not need rebuilding. Confirm during `/implement`.
- **Fix vs feature:** **fix** — all seven are COBOL-85 standard forms.

## 11. Open questions

- Q1: R2 — "each receiver from the original value" is the standard's rule for
  `ADD`/`SUBTRACT` receiver lists. Confirm the existing multi-receiver
  `COMPUTE`/`GIVING` implementation already does this, and reuse it.
- Q2: R11 — should `WITH LOCK` be enforced across processes? No: the
  supported-syntax doc already scopes locking to a single run unit, and CCVS85
  runs single-process. Enforce within the run unit only.
