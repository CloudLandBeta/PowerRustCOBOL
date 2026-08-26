# NIST-spec — Segmentation (section priority numbers, SEGMENT-LIMIT)

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **SG module, 13 programs, 0 clean today** — the only
  in-scope module scoring zero. 14 programs use a section priority number and 5
  use `SEGMENT-LIMIT`; OBIC1A uses one too.

## 1. Overview

COBOL-85 segmentation lets a section header carry a **priority number** 0-99:

```
001700 NUMBER1 SECTION 18.
001600 BEANO SECTION 1.
013000 SECT-IC219-0001 SECTION 30.
```

and `OBJECT-COMPUTER` may carry `SEGMENT-LIMIT IS integer`.

PowerRustCOBOL does not parse either. Measured:

```
L11   unexpected token in statement: IntegerLiteral(30)
      | 001100 SECT-IC219-0001 SECTION 30.
```

A grep for `SEGMENT-LIMIT` across the lexer, parser and AST returns nothing.

## 2. What segmentation means, and what it means here

Priority numbers were designed for machines that overlaid program segments in
limited core:

- **0-49** — *fixed* segments, always resident.
- **50-99** — *independent* segments, which may be overlaid, and which are
  restored to their **initial state** each time control enters them from a
  segment with a different priority.
- `SEGMENT-LIMIT IS n` moves the fixed/independent boundary.

On a modern host there is no overlaying, so the only *observable* rule is the
initial-state restoration for independent segments — and specifically its
interaction with `ALTER`: an altered `GO TO` inside an independent segment
reverts when the segment is re-entered from outside. CCVS85's SG module tests
exactly this, which is why `ALTER` appears 398 times in the distribution and
`ALTER` is already implemented (`alter_map` in the interpreter).

The pragmatic reading: **parse it fully, execute the one rule that is
observable.**

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall parse a section header with an optional
  priority number 0-99 (`section-name SECTION [priority].`).
- **R2 (ubiquitous):** The system shall parse `SEGMENT-LIMIT IS integer` in the
  `OBJECT-COMPUTER` paragraph.
- **R3 (ubiquitous):** The system shall record each section's priority, treating
  an omitted priority as 0.
- **R4 (state):** While a section's priority is at or above the segment limit
  (default 50), the system shall restore that section's `ALTER`-modified `GO TO`
  targets to their source-declared values whenever control enters the section
  from a section of a different priority.
- **R5 (constraint):** The system shall not reorder, relocate or otherwise
  change the execution order of sections. Priority affects state restoration
  only.
- **R6 (constraint):** The system shall not reject a priority number on any
  section, including sections in DECLARATIVES where the standard forbids it —
  rejecting it would be new strictness that no NIST program requires.
- **R7 (ubiquitous):** A section name composed entirely of digits shall be
  accepted (`0 SECTION.`) — see `NIST-spec-user-defined-words.md` R3.

## 4. Acceptance criteria

- [ ] AC1 — `NUMBER1 SECTION 18.` and `BEANO SECTION 1.` parse; the sections are
      reachable by `PERFORM` and `GO TO`.
- [ ] AC2 — `SEGMENT-LIMIT IS 30` parses and is recorded.
- [ ] AC3 — An `ALTER`ed `GO TO` inside a section with priority ≥ the segment
      limit reverts when the section is re-entered from a different priority,
      and does **not** revert when re-entered from the same priority.
- [ ] AC4 — A section with priority < the segment limit keeps its `ALTER`ed
      target across entries (no restoration).
- [ ] AC5 — SG301M … SG401M and OBIC1A parse, and the SG module rises from
      0 / 13, scored on each program's own `PASS`/`FAIL` report.
- [ ] AC6 — Existing programs with plain `X SECTION.` headers are unaffected.

## 5. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator emits unprioritised sections;
  R3's default of 0 keeps them fixed segments, so behaviour is unchanged. AC6
  pins it.
- **Docs:** `docs/cobol85-supported-syntax-en.md` gains segmentation. The
  Developer's Guide should say plainly that priority numbers are accepted for
  compatibility and that only the independent-segment state rule is observable —
  a PowerCOBOL/isCOBOL developer migrating old source needs to know it will
  compile, not that it will overlay.
- **Fix vs feature:** **fix** — standard COBOL-85, obsolete but in the standard.

## 6. Open questions

- Q1: R4 is the only executable semantics. Does the operator want it, or is
  parse-and-ignore enough? Parse-and-ignore gets SG programs *compiling* but
  they would then self-report `FAIL`, so AC5 would not be met. Recommendation:
  implement R4 — the `alter_map` already exists and the change is a save/restore
  around section entry.
- Q2: Does anything else need per-segment initial-state restoration besides
  `ALTER` targets? In COBOL-85, no — data items are not re-initialised. Confirm
  against the SG programs during `/plan`.
