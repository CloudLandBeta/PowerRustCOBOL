# NIST-spec — LINAGE, END-OF-PAGE, and the I-O-CONTROL paragraph

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** 5 programs use `LINAGE` (with `LINAGE-COUNTER` referenced
  ~40 times); 21 use `SAME … AREA`; 9 use `MULTIPLE FILE TAPE`. Concentrated in
  the SQ module (85 programs, 39 clean).

Two small, related DATA/ENVIRONMENT DIVISION gaps are grouped here because
neither justifies its own spec and both concern file declaration.

## Part 1 — LINAGE and END-OF-PAGE

### Overview

`LINAGE` declares a logical page on a sequential (print) file:

```
LINAGE IS n LINES
    [WITH FOOTING AT m]
    [LINES AT TOP t]
    [LINES AT BOTTOM b]
```

It brings the special register **`LINAGE-COUNTER`** — one per file, holding the
current line within the page body — and enables the `AT END-OF-PAGE` /
`NOT AT END-OF-PAGE` phrases on `WRITE`.

A grep for `LINAGE` across the lexer, parser and AST returns nothing. CCVS85
uses `LINAGE-COUNTER` directly (`MOVE LINAGE-COUNTER TO COMPUTED-18V0`,
`IF LINAGE-COUNTER EQUAL 1`), and has files named `FL3-LC`, `DL3-LC` for
checking it.

### Requirements (EARS)

- **R1:** The system shall parse the full `LINAGE` clause on an FD, with all
  four operands as either integers or data-names.
- **R2:** The system shall provide a `LINAGE-COUNTER` register per LINAGE file,
  qualifiable as `LINAGE-COUNTER OF file-name`, readable by the program and not
  writable by it.
- **R3 (event):** When a `WRITE` advances past the footing line, the system
  shall execute the `AT END-OF-PAGE` phrase; when it does not, `NOT AT
  END-OF-PAGE`.
- **R4 (event):** When a `WRITE … ADVANCING PAGE` occurs, or the page body is
  exhausted, the system shall reset `LINAGE-COUNTER` to 1 and emit the top and
  bottom margins.
- **R5 (constraint):** A file without a `LINAGE` clause shall behave exactly as
  today, with no `LINAGE-COUNTER` and no end-of-page processing.

### Acceptance criteria

- [ ] AC1 — `LINAGE IS 20 LINES WITH FOOTING AT 15 LINES AT TOP 2 LINES AT
      BOTTOM 3` parses with all four operands.
- [ ] AC2 — `LINAGE-COUNTER` reads 1 after the first `WRITE` to a fresh page.
- [ ] AC3 — `AT END-OF-PAGE` fires on the write that reaches the footing line,
      and not before.
- [ ] AC4 — `WRITE … ADVANCING PAGE` resets the counter and writes the margins.
- [ ] AC5 — Non-LINAGE sequential files are byte-identical to today
      (regression test against the existing `tests/cobol/fileio/` suite).

## Part 2 — the I-O-CONTROL paragraph

### Overview

`I-O-CONTROL` carries `SAME [RECORD | SORT | SORT-MERGE] AREA`, `MULTIPLE FILE
TAPE`, `RERUN` and `APPLY`. Measured usage: `SAME RECORD AREA` 11, `SAME AREA`
8, `SAME SORT AREA` 4, `SAME REC AREA` 1, `MULTIPLE FILE` 9, `RERUN` present.
None is parsed today.

`SAME AREA` and `SAME RECORD AREA` are the two with observable semantics:

- `SAME AREA` — the named files share one storage area, so **only one may be
  open at a time**.
- `SAME RECORD AREA` — the named files share one *record* area, so writing a
  record of one file makes it visible through the others' record descriptions.
  This is genuinely observable and CCVS85 checks it.

`MULTIPLE FILE TAPE` and `RERUN` describe physical tape reels and checkpoint
restart; on a modern host they have no observable effect.

### Requirements (EARS)

- **R6:** The system shall parse `SAME [RECORD | SORT | SORT-MERGE] AREA FOR
  file-1 file-2 …`, accepting the abbreviation `REC` for `RECORD`.
- **R7:** The system shall implement `SAME RECORD AREA` by giving the named
  files one shared record buffer.
- **R8 (event):** When a program opens a second file named in a `SAME AREA`
  clause while the first is open, the system shall report the standard file
  status rather than opening it.
- **R9:** The system shall parse `MULTIPLE FILE TAPE CONTAINS file-1 [POSITION
  n] …`, `RERUN ON … EVERY …` and `APPLY …`, and shall accept them as no-ops.
- **R10 (constraint):** `SAME SORT AREA` and `SAME SORT-MERGE AREA` shall be
  accepted as no-ops; the sort work buffer is already in memory and shares
  nothing observable.
- **R11 (constraint):** The system shall not change behaviour for programs with
  no `I-O-CONTROL` paragraph.

### Acceptance criteria

- [ ] AC6 — All four `SAME` spellings parse, including `SAME REC AREA`.
- [ ] AC7 — Two files under `SAME RECORD AREA` share a record buffer, observably.
- [ ] AC8 — Opening the second of two `SAME AREA` files while the first is open
      yields the standard file status, not a panic.
- [ ] AC9 — `MULTIPLE FILE TAPE`, `RERUN` and `APPLY` parse and do nothing.
- [ ] AC10 — The SQ module rises from 39 / 85, scored on program self-reports.

## Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** neither feature appears in generated code; R5 and
  R11 keep existing behaviour byte-identical.
- **Docs:** `LINAGE` belongs in the Developer's Guide — report pagination is
  ordinary work for the target audience, and its absence is a real gap. The
  I-O-CONTROL no-ops should be documented as accepted-but-inert so nobody
  expects tape semantics.
- **Fix vs feature:** **fix** — both are COBOL-85 standard.

## Open questions

- Q1: R8's file status for a `SAME AREA` conflict — COBOL-85 makes it undefined
  behaviour rather than naming a code. Recommendation: `41` (open of an already
  open file), which is the closest standard code and is what the CCVS85 programs
  are most likely to tolerate. Verify against the SQ programs during `/plan`.
- Q2: Does `LINAGE-COUNTER` need to be writable? The standard says no — the
  program may reference it but not modify it. Confirm no CCVS85 program writes
  to it.
