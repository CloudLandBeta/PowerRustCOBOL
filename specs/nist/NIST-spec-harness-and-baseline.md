# NIST-spec — CCVS85 harness and measured baseline

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Source of truth:** `NIST/newcob.val,cbl` — NIST CCVS85 VERSION 4.0, 01 OCT 1992
  (COBOL 85 VERSION 4.2, Apr 1993 SSVG), 28,210,031 bytes, 348,271 lines.

## 1. Overview

`newcob.val,cbl` is the official NIST COBOL-85 validation suite. It is **not a
program** — it is a distribution containing 512 members delimited by control
cards, of which **459 are COBOL programs** and **51 are COPY library members**.
Before any of it can be compiled it has to be split and *prepared*, and the
preparation rules are part of the suite, not part of the compiler.

This spec defines that harness and records the **measured baseline** every other
`NIST-spec-*.md` is sized against. It is the foundation document: each other
spec states its blast radius as "N of 459 programs", and those numbers all come
from the instrument defined here.

## 2. Goals / Non-goals

- **Goals:**
  - A reproducible way to split, prepare, compile and score the CCVS85 suite.
  - One canonical pass/fail number per run, per module, so progress on the other
    specs is measurable rather than asserted.
  - A record of the baseline as of 2026-08-25, so regressions are visible.
- **Non-goals:**
  - Fixing any language defect (that is what the other specs are for).
  - Byte-compatibility with the NIST-supplied `EXEC85` driver program. We
    reimplement the preparation rules; we do not run their COBOL installer.

## 3. The distribution's structure

Members are delimited by control cards in column 1:

```
*HEADER,COBOL,NC101A                    ← a test program
*HEADER,COBOL,ST101A,SUBPRG,ST103A      ← a called subprogram; its PROGRAM-ID is ST103A
*HEADER,CLBRY,ALTL1                     ← a COPY library member
*END-OF,NC101A
```

Counts: 459 `COBOL`, 51 `CLBRY`, 512 total.

Programs per module:

| Module | Programs | What it exercises |
|--------|---------:|-------------------|
| NC | 95 | Nucleus |
| SQ | 85 | Sequential I/O |
| IC | 47 | Inter-program communication |
| IF | 45 | Intrinsic functions |
| IX | 42 | Indexed I/O |
| ST | 40 | Sort/Merge |
| RL | 35 | Relative I/O |
| SM | 17 | Source text manipulation (COPY/REPLACE) |
| DB | 15 | Debug |
| SG | 13 | Segmentation |
| CM | 9 | Communication |
| RW | 6 | Report Writer |
| OBSQ / OBIC / OBNC | 4 / 3 / 2 | Obsolete-feature variants |
| EXEC | 1 | The CCVS driver program |

## 4. Preparation rules (EARS)

- **R1 (ubiquitous):** The harness shall split the distribution on `*HEADER,` /
  `*END-OF` control cards, taking the member name from field 3, or from field 5
  when field 4 is `SUBPRG`.
- **R2 (ubiquitous):** The harness shall discard columns 73-80 (the
  identification area) of every line before compiling. Every CCVS85 line carries
  a program stamp there (`NC1014.2`); leaving it in place makes every program
  fail. See `NIST-spec-fixed-format-reference-format.md`.
- **R3 (ubiquitous):** The harness shall normalise the CCVS *selector* letters in
  the indicator area (column 7). COBOL-85 defines only space, `*`, `/`, `-` and
  `D` there; CCVS85 additionally uses `Y P C S F R X A G J T B U H 6` to mark
  optional source lines the installer chooses. Census over the distribution:

  | Indicator | Lines | | Indicator | Lines |
  |---|---:|---|---|---:|
  | (space) | 303,292 | | `X` | 453 |
  | `*` | 29,921 | | `A` | 165 |
  | `Y` | 4,830 | | `G` | 133 |
  | `P` | 4,542 | | `/` | 47 |
  | `-` | 2,085 | | `J` | 36 |
  | `C` | 994 | | `D` | 32 |
  | `S` | 635 | | `T`,`B` | 28 each |
  | `F` | 514 | | `U` | 10 |
  | `R` | 512 | | `H` | 4, `6` | 3 |

  Two strategies are legitimate — *activate* (blank the selector, making the line
  ordinary source) or *drop* (remove the line). Both leave coherent programs;
  e.g. in NC101A the `S` lines add an `EXIT PROGRAM` / `TERMINATE-CALL` pair and
  the `Y` lines add a self-contained page-eject `IF` block.
  **Measured: both strategies score identically (199/459).** The harness shall
  default to *activate*, because it compiles strictly more source.
- **R4 (ubiquitous):** The harness shall treat `XXXXXnnn` words as ordinary
  COBOL words. They are the suite's implementor-name placeholders (`XXXXX055` =
  system printer, `XXXXX082/083` = source/object computer, `XXXXX062` etc. =
  file assignments). They are syntactically valid user-defined words, so parsing
  needs no substitution; **execution does**, and the substitution table is a
  run-phase concern of this spec, not a language concern.
- **R5 (constraint):** The harness shall not modify `NIST/newcob.val,cbl`. It is
  the source of truth and is treated as read-only.
- **R6 (event):** When a member is a `CLBRY` library member, the harness shall
  make it available to the COPY preprocessor under its member name rather than
  compiling it as a program. 326 of the 459 programs contain a `COPY` statement.

## 5. Scoring

- **R7 (ubiquitous):** The harness shall report, per run: programs analysed,
  programs with zero front-end errors, the same split per module, and a
  root-cause census (the first line-anchored diagnostic of each failing
  program, bucketed).
- **R8 (constraint):** A program shall not be counted as passing merely because
  it parses. Front-end cleanliness is *stage 1*. Stage 2 is execution: a CCVS85
  program prints its own `PASS`/`FAIL` tally, and the harness shall compare that
  report rather than the exit status.
  This matters because of literal continuation
  (`NIST-spec-literal-continuation.md`): a corrupted literal can leave a program
  parsing cleanly while holding the wrong data.

## 6a. Current baseline — 2026-08-25, after the source-format fix

`NIST-spec-fixed-format-reference-format.md` and
`NIST-spec-literal-continuation.md` shipped on 2026-08-25. The compiler now
applies the classic reference format itself, so the harness hands it the
**untouched** distribution and prepares nothing:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

| Pass | Preparation | Clean / 459 |
|------|-------------|------------:|
| `raw` | none, relaxed `Fixed` | 0 (0.0 %) |
| `strict` | none — `SourceFormat::FixedStrict` does it all | **224 (48.8 %)** |

Scored the way §5 R7/R8 asks — in scope, excluding the 25 N/A programs.
**Current, at 1.62.11** (after `NIST-spec-identification-division-comment-entries.md` landed):

```
--- PASS / FAIL / N-A ---
  PASS  241 / 434   (55.5% of the in-scope suite)
  FAIL  193 / 434   (44.5%)
  N-A    25 / 459   (out of RustCOBOL scope: CM, RW, OB*, EXEC85)
```

| Version | In-scope PASS | Landed |
|---|---:|---|
| 1.62.7 | 0 / 434 | — |
| 1.62.8 | 222 / 434 | fixed-format + literal continuation |
| 1.62.10 | 237 / 434 | numeric literals with a leading `.` |
| **1.62.11** | **241 / 434** | IDENTIFICATION comment-entry paragraphs |

| Module | Clean / Total | | Module | Clean / Total |
|--------|--------------:|---|--------|--------------:|
| NC | 29 / 95 | | SM | 4 / 17 |
| SQ | 47 / 85 | | DB | 9 / 15 |
| IC | 32 / 47 | | SG | 0 / 13 |
| IF | 29 / 45 | | CM | 0 / 9 |
| IX | 31 / 42 | | RW | 0 / 6 |
| ST | 30 / 40 | | OBSQ | 0 / 4 |
| RL | 30 / 35 | | OBIC | 2 / 3 |
| | | | OBNC / EXEC | 0 / 2, 0 / 1 |

Overall clean is **243 / 459**; 2 of those are in the out-of-scope OBIC module,
so the in-scope figure is **241 / 434 (55.5 %)**.

**R3 is no longer needed.** `FixedStrict` reads a non-standard column-7
character as ordinary source, so the CCVS selector letters need no harness
normalisation — which is why 224 beats the 199 that harness preparation reached.
The `nist` / `nistdel` / `col72` passes are retained only to reproduce the
pre-fix measurements below.

Root causes remaining, first line-anchored error per failing program:

| Programs | Root cause | Spec |
|---:|---|---|
| 32 | IDENTIFICATION comment entry | `NIST-spec-identification-division-comment-entries.md` |
| 29 | separator comma | `NIST-spec-separators.md` |
| 14 | `FUNCTION … (ALL)` | `NIST-spec-intrinsic-function-gaps.md` |
| 12 | `WHEN` signed literal `THRU` | `NIST-spec-statement-grammar-gaps.md` |
| 11 | space-separated subscripts | `NIST-spec-separators.md` |
| 10 | `SET SW-1 TO ON`, `SET A, B, C TO 1` | `NIST-spec-special-names.md`, `NIST-spec-separators.md` |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | `NIST-spec-statement-grammar-gaps.md` |
| 7 | `COPY` position / split across lines | `NIST-spec-copy-and-replace.md` |
| 6 | `DATE-COMPILED` (was 48 before the fix) | `NIST-spec-identification-division-comment-entries.md` |
| 5 | separator **semicolon** — `START IX-FD1 ; INVALID KEY` | `NIST-spec-separators.md` |
| 4 | `OCCURS` integer on a following line | `NIST-spec-separators.md` |

The numeric-literal bucket that led this table at 1.62.8 with 36 programs is
gone; the `SET` bucket that replaced it was invisible then, because those
programs failed earlier on the literal. The semicolon bucket appeared the same
way after the column rule landed. Expect more such reordering — re-read this census
after every one rather than trusting the previous ranking.

## 6. Measured baseline — 2026-08-25, version 1.62.1 (before the fix)

Instrument: `crates/cobolt-semantic/examples/nist_conformance.rs`
(lexer → parser → semantic analyser; no execution).

```bash
cargo run -p cobolt-semantic --example nist_conformance -- nist
```

| Pass | Preparation | Clean / 459 |
|------|-------------|------------:|
| `raw` | none — source handed over as-is with `SourceFormat::Fixed` | **0 (0.0 %)** |
| `col72` | columns 73-80 discarded | 39 (8.5 %) |
| `nist` | + CCVS indicator selectors normalised (activate) | **199 (43.4 %)** |
| `nistdel` | + CCVS indicator selectors normalised (drop) | 199 (43.4 %) |

Per module, at the `nist` pass:

| Module | Clean / Total | | Module | Clean / Total |
|--------|--------------:|---|--------|--------------:|
| NC | 22 / 95 | | SM | 4 / 17 |
| SQ | 39 / 85 | | DB | 5 / 15 |
| IC | 32 / 47 | | SG | 0 / 13 |
| IF | 21 / 45 | | CM | 0 / 9 |
| IX | 27 / 42 | | RW | 0 / 6 |
| ST | 17 / 40 | | OBSQ | 0 / 4 |
| RL | 30 / 35 | | OBIC | 2 / 3 |
| | | | OBNC | 0 / 2 |
| | | | EXEC | 0 / 1 |

Excluding the 25 programs held out of scope
(`NIST-spec-out-of-scope-modules.md`), of which 2 are already clean, the
in-scope baseline is **197 / 434 (45.4 %)**.

**The `raw` figure is the headline.** With no preparation the suite scores
0/459 — not because of 459 language gaps, but because two mechanical rules
(column 73-80, indicator column) are unimplemented. Those are specs 2 and 3.

## 7. Construct census

How many of the 459 programs contain each construct, so every other spec can
state its blast radius honestly:

| Programs | % | Construct | Spec |
|---------:|---:|-----------|------|
| 396 | 86.3 | literal continuation (`-` in col 7 continuing a literal) | `NIST-spec-literal-continuation.md` |
| 95 | 20.7 | comma used as an operand separator | `NIST-spec-separators.md` |
| 48 | 10.5 | numeric literal with a leading `.` | `NIST-spec-numeric-literals.md` |
| 45 | 9.8 | intrinsic `FUNCTION` | `NIST-spec-intrinsic-function-gaps.md` |
| 37 | 8.1 | `ORGANIZATION RELATIVE` | `NIST-spec-relative-organization.md` |
| 34 | 7.4 | `INSTALLATION` paragraph | `NIST-spec-identification-division-comment-entries.md` |
| 34 | 7.4 | `SECURITY` paragraph | ” |
| 21 | 4.6 | `SAME … AREA` | `NIST-spec-linage-and-io-control.md` |
| 19 | 4.1 | `ALPHABET` clause | `NIST-spec-special-names.md` |
| 15 | 3.3 | `USE FOR DEBUGGING` | `NIST-spec-debugging-module.md` |
| 14 | 3.1 | `SECTION` with a priority number | `NIST-spec-segmentation.md` |
| 14 | 3.1 | `WITH DEBUGGING MODE` | `NIST-spec-debugging-module.md` |
| 13 | 2.8 | `END PROGRAM` | `NIST-spec-nested-programs.md` |
| 11 | 2.4 | `COMMUNICATION SECTION` | `NIST-spec-out-of-scope-modules.md` |
| 11 | 2.4 | `FUNCTION` argument `(ALL)` | `NIST-spec-intrinsic-function-gaps.md` |
| 9 | 2.0 | `MULTIPLE FILE TAPE` | `NIST-spec-linage-and-io-control.md` |
| 7 | 1.5 | `REPLACE` statement | `NIST-spec-copy-and-replace.md` |
| 7 | 1.5 | `REPORT SECTION` | `NIST-spec-out-of-scope-modules.md` |
| 6 | 1.3 | `CLOSE … WITH LOCK` | `NIST-spec-statement-grammar-gaps.md` |
| 5 | 1.1 | `LINAGE` clause | `NIST-spec-linage-and-io-control.md` |
| 5 | 1.1 | `SEGMENT-LIMIT` | `NIST-spec-segmentation.md` |
| 5 / 4 | 1.1 / 0.9 | `EXTERNAL` / `GLOBAL` | `NIST-spec-nested-programs.md` |
| 3 / **0** | 0.7 / — | `DATE-COMPILED` / `REMARKS` paragraph — **`REMARKS` re-measured 2026-08-25: zero real paragraphs; the original count matched `VALUE "REMARKS"` inside a literal** | `NIST-spec-identification-division-comment-entries.md` |
| 2 | 0.4 | `CURRENCY SIGN` | `NIST-spec-special-names.md` |
| 2 | 0.4 | `INSPECT … CONVERTING` | `NIST-spec-statement-grammar-gaps.md` |
| 1 | 0.2 | `DECIMAL-POINT IS COMMA` | `NIST-spec-special-names.md` |

**Correction — `COPY`.** A first pass of this census reported "326 programs
(71 %) use `COPY`". That was wrong: it counted the word inside the CCVS
copyright banner literal, `"… COPY - NOT FOR DISTRIBUTION"`, which appears 324
times. Counting only real directives over non-comment source, columns 8-72:

| Occurrences | Directive |
|---:|---|
| 91 | `COPY <name>` on one line |
| 39 | `COPY` with the name on a following line |
| 112 | `COPY … REPLACING` |
| 10 | `REPLACE == … ==` |

The **SM module (17 programs)** is what systematically tests source text
manipulation. Any census filter must exclude literal text — a lesson that
applies to the `SORT `, ` EXTERNAL` and `CURRENCY` rows above too, which are
substring matches and should be treated as upper bounds until re-measured with
a literal-aware scan.

## 8. Acceptance criteria

- [ ] AC1 — The harness splits the distribution into 459 COBOL programs and 51
      library members, with `SUBPRG` members named by their own `PROGRAM-ID`.
- [ ] AC2 — Preparation is implemented per R2/R3 and is *selectable*, so a run
      can reproduce the `raw`, `col72` and `nist` figures in §6 exactly.
- [x] AC3 — ✅ **done 2026-08-25.** A run prints the PASS / FAIL / N-A split,
      the per-module table with out-of-scope modules marked `N-A`, and the
      root-cause census.
- [ ] AC4 — The baseline in §6 is reproducible on an unmodified checkout, and is
      re-recorded in this file whenever a `NIST-spec-*` lands.
- [ ] AC5 — Stage 2 (execution + `PASS`/`FAIL` report comparison) is available
      for at least the NC module before any spec claims a module "passes".

## 9. Constraints & steering check

- **i18n:** none — the harness is a developer instrument, not IDE UI.
- **Generated-code contract:** none.
- **Docs:** `docs/cobol85-supported-syntax-en.md` currently states "The COBOL-85
  verb / clause set is **fully covered**". That claim does not survive contact
  with CCVS85 and must be corrected as the other specs land (GOLDEN RULE #3).
- **Fix vs feature:** **fix.** Every gap CCVS85 exposes is missing or
  non-conformant COBOL-85 behaviour, which CLAUDE.md rule #4 classifies as
  technical debt — a fix, announced on forum f=97, not f=96.
- **PRIME DIRECTIVE:** the harness is Rust, living as a cargo example. No shell
  or scripting language is introduced to split or prepare the suite.

## 10. Open questions

- Q1: Stage 2 needs `XXXXXnnn` substitution values (printer, file assignments,
  source/object computer). Do we adopt the NIST-suggested defaults or map them
  onto PowerRustCOBOL paths? — **operator ruling needed.**
- Q2: Do we vendor the split programs into `tests/cobol/nist/` (348k lines,
  ~28 MB) or split on the fly each run? Splitting on the fly keeps the repo
  small and keeps `newcob.val,cbl` the single source of truth; that is the
  recommendation.
- Q3: GOLDEN RULE #7 requires new tests to report quantified results. The
  harness already reports counts and per-module splits; confirm that satisfies
  the rule for a suite this size, rather than per-program timing.
