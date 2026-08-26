# NIST-spec — RELATIVE file organization

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **RL module, 35 programs**, plus RELATIVE usage in
  OBSQ/ST. Measured: 50 `ORGANIZATION … RELATIVE` clauses and 20+
  `RELATIVE KEY IS …` clauses across the distribution.
- **Note:** RL scores **30 / 35 at the front end** today — the highest of any
  module. That number is misleading and is exactly why this spec exists.

## 1. Overview

`ORGANIZATION IS RELATIVE` parses. `FileOrganization::Relative` exists in
`crates/cobolt-ast/src/program.rs:109` and the parser sets it at
`crates/cobolt-parser/src/parser.rs:702`.

**Nothing in the runtime ever matches that variant.** A grep for `Relative`
across `crates/cobolt-runtime/src/` returns nothing. CLAUDE.md already records
this as a trap:

> **RELATIVE is a trap:** the parser *accepts* `ORGANIZATION IS RELATIVE` […],
> but **nothing in the runtime ever matches that variant** — there is no
> dispatch, no engine, no diagnostic. A program that declares it parses and then
> misbehaves silently.

So 30 of 35 RL programs "pass" the front end and would then produce wrong
results with no error at all. This is the single clearest demonstration of why
`NIST-spec-harness-and-baseline.md` R8 forbids scoring on parse success.

## 2. Goals / Non-goals

- **Goals:** a working RELATIVE file organization — sequential, random and
  dynamic access — sufficient for the RL module to run and self-report `PASS`.
- **Non-goals:**
  - A new on-disk container format if an existing one can carry it. See §4.
  - Cross-process locking (already out of scope project-wide).

## 3. What RELATIVE requires

A relative file is a numbered sequence of fixed-length record slots, addressed
by an integer **relative record number** starting at 1. A slot is either
occupied or empty; reading an empty slot is not an error at the file level, it
is a "record not found" (file status 23).

| Verb | RELATIVE semantics |
|---|---|
| `OPEN INPUT/OUTPUT/I-O/EXTEND` | as for sequential, plus slot addressing |
| `READ … NEXT` | next *occupied* slot; sets the RELATIVE KEY item |
| `READ` (random) | the slot named by the RELATIVE KEY item; 23 if empty |
| `WRITE` | ACCESS SEQUENTIAL: next slot. RANDOM/DYNAMIC: the slot named by the key; 22 if occupied |
| `REWRITE` | replaces the slot's record; 23 if empty |
| `DELETE` | empties the slot; 23 if empty |
| `START` | positions by relative record number with `=`, `>`, `NOT <` |

`RELATIVE KEY IS data-name` names an unsigned integer item in WORKING-STORAGE
(not in the record) that carries the record number in both directions.

## 4. Implementation direction

PowerRustCOBOL already has three INDEXED engines and a `RecordLayout` mechanism
in `crates/cobolt-runtime/src/files.rs`. A relative file is a strictly simpler
structure than an indexed one: a fixed-size slot array with an occupancy bit.

Two candidate approaches for `/plan` to choose between:

1. **A dedicated relative engine** — a flat file of `record_len + 1` byte slots
   (occupancy flag plus record), with the relative record number as the offset.
   Direct, small, and gives true O(1) random access.
2. **Reuse an existing store** keyed by the relative record number, via the
   `IndexedStore` trait in `crates/cobolt-runtime/src/indexed.rs`.

Option 1 is the recommendation: it is a genuinely different access model, and
building it on the indexed engines would pay B-tree costs for what is array
indexing. It also matches the `OpenFile` enum's existing shape
(`Reader`/`Writer`/`Indexed` gains `Relative`).

## 5. Requirements (EARS)

- **R1 (ubiquitous):** The system shall dispatch `OPEN`, `CLOSE`, `READ`,
  `WRITE`, `REWRITE`, `DELETE` and `START` on `FileOrganization::Relative` to a
  relative-file engine.
- **R2 (ubiquitous):** The system shall parse and honour `RELATIVE KEY IS
  data-name`, writing the record number into it on sequential `READ` and reading
  it on random access.
- **R3 (ubiquitous):** The system shall support `ACCESS MODE IS SEQUENTIAL`,
  `RANDOM` and `DYNAMIC`.
- **R4 (ubiquitous):** The system shall return the standard file status codes —
  `00`, `02`, `10`, `22`, `23`, `35`, `49` — matching the INDEXED engine's
  existing `status` module conventions.
- **R5 (ubiquitous):** The system shall honour `INVALID KEY` / `NOT INVALID KEY`
  and `AT END` / `NOT AT END` phrases, reusing the existing `run_key_outcome`
  path.
- **R6 (event):** When a program declares `ORGANIZATION IS RELATIVE` and the
  engine is not yet available, the system shall emit a clear diagnostic rather
  than running silently. This requirement stands **even if R1 is deferred** —
  the silent-misbehaviour trap must close first.
- **R7 (constraint):** The system shall not change SEQUENTIAL, LINE SEQUENTIAL
  or INDEXED behaviour.

## 6. Acceptance criteria

- [ ] AC1 — R6 lands first and independently: a RELATIVE program that cannot be
      executed says so.
- [ ] AC2 — Write 1,000 records to slots 1-1000 in RANDOM mode, read them back
      in SEQUENTIAL mode, and get 1,000 records in order.
- [ ] AC3 — `DELETE` a slot, then `READ` it randomly → file status 23; a
      sequential scan skips it.
- [ ] AC4 — `WRITE` to an occupied slot in RANDOM mode → file status 22.
- [ ] AC5 — `START … KEY IS > n` positions correctly, and `READ NEXT` continues
      from there.
- [ ] AC6 — The RL module runs and self-reports; the score is taken from each
      program's own `PASS`/`FAIL` report, not from parse success.
- [ ] AC7 — A new `tests/cobol/` program covers RELATIVE and reports quantified
      results per GOLDEN RULE #7: records written, elapsed time and rec/s for
      each phase (WRITE, random READ, REWRITE, DELETE, sequential scan).
- [ ] AC8 — `docs/cobol85-supported-syntax-en.md` avoid-list item 4 ("RELATIVE
      file organization") is removed, and CLAUDE.md's "RELATIVE is a trap" note
      is updated.

## 7. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the IDE's `IndexedFile` control and `.cidx`
  editor are INDEXED-only; RELATIVE is CLI/runtime-level and adds no IDE
  surface. If a designer-level RELATIVE control is ever wanted, that is a
  separate **feature**.
- **Docs:** the Developer's Guide gains a RELATIVE section — a
  PowerCOBOL/isCOBOL developer will expect it alongside the indexed-file
  chapter, and its absence is conspicuous.
- **System KB:** no control/property/event change, so `chunked.data` is
  untouched. Confirm during `/implement`.
- **Fix vs feature:** **fix.** RELATIVE is COBOL-85 standard organization and
  the parser already advertises it; completing it is technical debt (CLAUDE.md
  rule #4), announced on f=97.

## 8. Open questions

- Q1: Engine choice (§4). Recommendation: option 1, a dedicated slot-array
  engine.
- Q2: Should `STORAGE IS MEMORY | DISK` — the PowerRustCOBOL SELECT extension —
  apply to RELATIVE too? Recommendation: yes, for symmetry with INDEXED, with
  DISK the default as it is there.
- Q3: Variable-length records with RELATIVE (`RECORD IS VARYING`). NIST's RL
  module includes some; confirm during `/plan` whether the slot array needs a
  length prefix.
