# NIST-spec — COPY and REPLACE (source text manipulation, SM module)

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **SM module, 17 programs, 4 clean today**. Measured
  directive counts over the distribution: 91 `COPY <name>` on one line, 39
  `COPY` with the name on a later line, 112 `COPY … REPLACING`, 10 `REPLACE`.
  The suite also ships **51 `CLBRY` library members** that exist only to be
  copied.

## 1. Overview

PowerRustCOBOL already has a COPY/REPLACE preprocessor —
`crates/cobolt-lexer/src/copybook.rs` — which expands `COPY name [OF lib]
[REPLACING a BY b …].` and `REPLACE a BY b … .` / `REPLACE OFF.` before
tokenization. The SM module nonetheless scores 4 / 17, for three reasons.

### 1a. The copybook library is a set of members, not a directory of files

`expand_copybooks(source, base_dir, format)` resolves a copybook by name
relative to a **filesystem directory**. CCVS85's library members live inside the
distribution as `*HEADER,CLBRY,<name>` sections. The harness
(`NIST-spec-harness-and-baseline.md` R6) must present those 51 members to the
preprocessor, which means the resolver needs a source of copybooks that is not
"a file on disk next to the program".

### 1b. `COPY` is not recognised in every position CCVS85 uses

Measured root causes in 7 SM programs:

| Program | Source | Diagnostic |
|---|---|---|
| SM101A | `                                                  COPY K1PRA.` | `unexpected token in statement: Identifier("COPY")` |
| SM201A | `                                                  COPY K1PRB` (no period; continues) | ” |
| SM202A | `                                                  COPY K2SEA` | ” |
| SM206A | `     COPY                                                KP001` | ” |

Two distinct problems: the directive starting deep in Area B, and the directive
split across lines with the copybook name on the following line. COBOL-85 allows
both — `COPY` is a *statement* whose operands follow normal continuation rules,
and it may appear anywhere a word may appear.

That `COPY` reaches the parser as `Identifier("COPY")` at all shows the
preprocessor did not consume it, so this is a preprocessor scan problem, not a
parser problem.

### 1c. The preprocessor inherits the continuation defect

`copybook.rs:42` flattens fixed-format text with `flatten_fixed`, the same
function that does not join continuation lines
(`NIST-spec-literal-continuation.md` §6). A copybook or a program containing a
continued literal is therefore corrupted **before** COPY expansion runs. SM103A
already shows a downstream symptom: `01 S-N-1 PICTURE 9(8)V99 VALUE IS
12345678,91.` is reported as an undeclared decimal comma, which suggests the
`DECIMAL-POINT IS COMMA` clause that should have arrived via a copybook did not.

**Dependency: this spec must land after `NIST-spec-literal-continuation.md`.**

## 2. Goals / Non-goals

- **Goals:** make COPY and REPLACE conform for the SM module — position,
  continuation, `REPLACING` with pseudo-text, nested COPY, and library
  resolution from the CCVS85 distribution.
- **Non-goals:**
  - Changing how PowerRustCOBOL projects resolve their own copybooks from disk.
    The library abstraction is *added*, the file path behaviour is kept.
  - `COPY` inside `EXEC RUST` blocks.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall recognise a `COPY` directive wherever it
  appears in Area B, including at column 55 and beyond.
- **R2 (ubiquitous):** The system shall accept a `COPY` directive whose
  copybook-name, `OF`/`IN` library-name, `REPLACING` operands or terminating
  period fall on subsequent lines.
- **R3 (ubiquitous):** The system shall support `COPY name REPLACING
  ==pseudo-text-1== BY ==pseudo-text-2==`, identifier-by-identifier, and
  literal-by-literal forms, applying every replacement to the copied text only.
- **R4 (ubiquitous):** The system shall support `REPLACE ==a== BY ==b==.` and
  `REPLACE OFF.` applying to the source text that follows, across statement and
  paragraph boundaries. Measured gap: SM208A's `REPLACE ==AO== BY ==TO==`
  reaches the parser as `Identifier("REPLACE")`.
- **R5 (ubiquitous):** The system shall provide a copybook **resolver
  abstraction** so a library member can be supplied from an in-memory set (the
  CCVS85 `CLBRY` members) as well as from a directory.
- **R6 (ubiquitous):** The preprocessor shall apply continuation joining before
  scanning for directives, so a directive or a copied literal split across lines
  is handled correctly.
- **R7 (ubiquitous):** The system shall support nested `COPY` (a copybook that
  itself contains `COPY`), keeping the existing `MAX_DEPTH` cycle guard.
- **R8 (constraint):** The system shall not alter the on-disk copybook
  resolution used by existing PowerRustCOBOL projects.
- **R9 (constraint):** Pseudo-text replacement shall be word-boundary aware, so
  `==AO==` does not rewrite the middle of `RATIO`.

## 4. Acceptance criteria

- [ ] AC1 — SM101A, SM201A, SM202A and SM206A expand their copybooks and parse
      clean.
- [ ] AC2 — SM208A's `REPLACE ==AO== BY ==TO==` rewrites the following text and
      `REPLACE OFF.` stops it.
- [ ] AC3 — All 51 `CLBRY` members resolve when the harness supplies them.
- [ ] AC4 — A copybook containing a continued literal expands with the literal
      intact (depends on `NIST-spec-literal-continuation.md`).
- [ ] AC5 — SM103A's `DECIMAL-POINT IS COMMA` arrives via its copybook and
      `12345678,91` reads as a decimal literal.
- [ ] AC6 — Existing project copybook tests still pass; on-disk resolution is
      byte-identical to today.
- [ ] AC7 — The SM module rises materially from 4 / 17 in the harness census.
- [ ] AC8 — R9: a replacement of `==AO==` leaves `RATIO` untouched.

## 5. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** generated form `.cbl` files do not use COPY, but
  hand-written Common Code may; R8 and AC6 protect it.
- **Docs:** the Developer's Guide should document copybooks properly — a
  PowerCOBOL/isCOBOL developer arrives with a large copybook estate and this is
  one of the first things they look for.
- **Fix vs feature:** **fix** — R5's resolver abstraction is new code, but it
  exists to make an existing standard feature work, which CLAUDE.md rule #4
  classifies as technical debt.

## 6. Open questions

- Q1: Should the resolver be a trait (`CopyLibrary`) or a closure? A trait is
  easier for the IDE to implement later against project files held in memory.
- Q2: `REPLACE` is a *source-level* directive whose scope crosses program
  boundaries within a compilation unit. Confirm the current implementation's
  scope rules against SM208A before changing them.
- Q3: The 39 `COPY`-with-name-on-a-later-line cases — are any of them inside a
  copybook rather than a program? That affects whether R2 must work recursively
  (it should).
