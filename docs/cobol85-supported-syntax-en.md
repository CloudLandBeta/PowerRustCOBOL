<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 Supported Syntax Reference

**What this document is for:** to say how much of the COBOL‑85 standard
RustCOBOL really implements — and to prove it against the **official NIST
COBOL‑85 validation suite** rather than assert it. The
[scoreboard](#-conformance-is-measured-not-asserted--nist-ccvs85) below is the
headline; everything after it is the detail behind that number.

**Ground truth of what the RustCOBOL lexer/parser/runtime actually accept today**,
derived from the source (`cobolt-lexer`, `cobolt-parser`, `cobolt-runtime`) and
checked against `NIST/newcob.val,cbl`.
Write tests against the ✅ forms; the ❌ forms will fail to parse or are no‑ops,
and ⚠️ forms parse but behave partially. This is the companion to
[`cobol85-verb-test-matrix-en.md`](cobol85-verb-test-matrix-en.md): the matrix says
*what* to test, this says *which spelling RustCOBOL understands*.

Legend: ✅ supported · ⚠️ parses but partial/simplified · ❌ not recognized
(avoid, or test only to confirm the gap).

---

## ★ Conformance is measured, not asserted — NIST CCVS85

**This is the point of the document.** Every claim below is checked against the
**official NIST COBOL‑85 validation suite** — CCVS85 version 4.0 (01 OCT 1992,
COBOL 85 version 4.2, Apr 1993 SSVG), the suite the United States National
Institute of Standards and Technology used to certify COBOL compilers. It is
28 MB, 348,271 lines, **459 COBOL programs** and 51 copybook members, and it
lives in this repository at `NIST/newcob.val,cbl`.

It is the source of truth. Where RustCOBOL and CCVS85 disagree, **CCVS85 is
right and RustCOBOL is wrong**, and the gap is recorded as a defect in
[`specs/nist/`](../specs/nist/README.md) — one spec per fix, with the failing
programs named.

### The scoreboard

Measured 2026‑08‑27 at version 1.62.22, on the untouched distribution:

| | Programs | Share | Meaning |
|---|---:|---:|---|
| ✅ **PASS** | **418** | **96.3 %** | of the 434 in‑scope programs |
| ❌ **FAIL** | **16** | 3.7 % | of the 434 in‑scope programs |
| ⬜ **N/A** | **25** | — | modules outside RustCOBOL's scope (below) |
| | **459** | | total programs in the suite |

Reproduce it:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ Compiling is the weaker claim

The table above counts programs the **front end accepts**. It does not say they
run. The suite scores itself — every CCVS85 program prints its own
`PASS` / `FAIL*` report — so there is a second, strictly stronger number: how
many run to completion and report **zero failures**.

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

Both numbers are reported per module, and never conflated:

| Module | Compile | Execution (0 failures) |
|---|---:|---:|
| **NC (Nucleus)** | **94 / 95** | **65 / 95** |

Work proceeds **one module at a time**: NC is finished only when both numbers
reach 95, and no other module is worked on until it does. A broad compile score
across ten modules says nothing about whether any of them work.

##### Four NC members can never reach "0 failures" as the harness scores it

The execution score counts a program clean when its **own CCVS report** shows
no failures. Five NC members print no such report, and not because anything is
broken:

| Member | Why it cannot be scored this way |
|---|---|
| **NC302M**, **NC303M**, **NC401M** | *Flagging* tests. They carry no `PASS`/`FAIL` machinery at all — each ends `TOTAL NUMBER OF FLAGS EXPECTED = n`, and the result being validated is the set of **diagnostics the compiler emits** for obsolete constructs. |
| **NC110M** | Writes its report with `DISPLAY`, to the operator's console, not to the CCVS print file the harness reads. |
| **NC109M** | Tests Format 1 `ACCEPT`, which reads from the operator. The validator is expected to supply the input; the harness runs with no stdin, so every comparison fails. |

Scoring these needs either operator input or a diagnostic-count check, both of
which are harness work rather than compiler work. Until that lands, the
attainable NC execution ceiling is **91 / 95**, and the target should be read
that way.

The comparable case that **was** settled is the external switch. NC174A, NC253A
and NC254A test `ON STATUS` / `OFF STATUS` against a switch the operator sets
before the run — nothing inside COBOL can set one — so the harness now passes
`--switch XXXXX051=ON --switch XXXXX052=OFF` (and the substituted `SWITCH-1` /
`SWITCH-2` spellings) exactly as the CCVS85 run instructions require. That is
configuration the validation procedure calls for, not a thumb on the scale: a
program that declares no switch is unaffected.

#### ⚠️ What PASS actually means — read this before quoting the number

A program counts as **PASS** when it goes through the RustCOBOL front end —
lexer, parser, semantic analyser — with **zero errors**, using
`--source-format=fixed`.

That is *compilation* conformance. It is **not** proof the program computes the
right answer. A CCVS85 program also prints its own `PASS`/`FAIL` tally when it
runs, and scoring that output is the **next stage** of this work — it is not
included in the 332 — see the execution scoreboard below. Two measured cases show why the distinction matters:

- 30 of the 35 RELATIVE‑file programs compile cleanly, and the runtime has **no
  RELATIVE engine at all** — they would run and produce wrong results silently.
- A literal continued across two lines can reassemble incorrectly and still
  parse, leaving the program holding the wrong data.

So: **PASS = "RustCOBOL accepts every construct in this program."** Nothing more,
yet.

#### 🔴 The execution scoreboard — the number that means "it works"

Everything above measures **compilation**. A CCVS85 program also *runs* and
prints its own `PASS`/`FAIL` tally, and that tally is what the suite exists to
produce. Since 1.62.15 the harness runs them:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

Measured 2026‑08‑27 at 1.62.23, **for NC only** — under GOLDEN RULE #9 no other
module is worked on until NC reaches its ceiling on both axes:

| | Programs |
|---|---:|
| in scope | 95 |
| did not compile | 1 |
| ran to completion | 89 |
| **…reporting 0 failures** | **65** |
| …reporting failures | 24 |
| ran but printed no report | 3 |
| timed out (>20 s) | 2 |
| crashed or were refused by the runtime | 0 |

The assertions the programs themselves report: **4 278 PASS / 226 FAIL**,
95.0 % of 4 504 scored.

> A `FAIL*` detail line is written **twice** on purpose — CCVS's `PRINT-DETAIL`
> runs `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — while `PASS ` is
> written once. Any raw marker count taken from the print file has to halve the
> failures before it means anything.

To read *why* a program fails, a third pass prints the failure detail its own
report carries, ready to bucket across a whole module:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> This is why the compilation figure is always reported as "RustCOBOL **accepts**
> these constructs". Quoting it as a conformance level would be wrong.

#### Per module

| Module | What it tests | PASS / Total | |
|---|---|---:|---|
| NC | Nucleus | 94 / 95 | NC108M — the implementor-defined editing picture `<(3),<<<.99` |
| SQ | Sequential I/O | 83 / 85 | the `OPEN` mode written on a continuation line |
| IC | Inter‑program communication | 45 / 47 | `END-CALL` reaches the statement dispatcher instead of being consumed by its `CALL`; one subscripted condition-name |
| IF | Intrinsic functions | **45 / 45** | ✅ complete |
| IX | Indexed I/O | **42 / 42** | ✅ complete |
| SG | Segmentation | **13 / 13** | ✅ complete |
| ST | Sort / Merge | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | Relative I/O | 34 / 35 | ⚠️ **compiles only — no runtime engine.** `ORGANIZATION IS RELATIVE` parses and is never matched at run time, so this row overstates real capability. The one failure is a dangling `ELSE` |
| SM | Source text manipulation (COPY/REPLACE) | 14 / 17 | a `$` inside a data-name; qualified/subscripted pseudo-text; a `PERFORM … VARYING` form |
| DB | Debug | 11 / 15 | `GO-TO` used as a user-defined word, colliding with the `GO TO` keyword pair; one program uses the Communication verb `DISABLE` |
| **In scope** | | **419 / 434** | |
| CM | Communication | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | Obsolete‑feature flagging | — | ⬜ N/A |
| EXEC85 | NIST's own COBOL driver program | — | ⬜ N/A |

### ⬜ N/A — what is out of RustCOBOL's scope, and why

These 25 programs are **not counted as failures**. They are features RustCOBOL
does not implement and is not planning to. Full reasoning in
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Module | Programs | Why it is out of scope |
|---|---:|---|
| **CM** — Communication | 9 | `COMMUNICATION SECTION`, `CD` entries, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Targets 1980s teleprocessing monitors — message queues owned by a transaction manager. There is no such runtime here, and the module was removed from later COBOL standards. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, `RD` entries, `INITIATE` / `GENERATE` / `TERMINATE`, control breaks. A large declarative sub‑language; PowerRustCOBOL's answer to reporting is the Form Designer and PDF export. Could become a *feature* later if wanted — it is the one exclusion with real user value. |
| **OBSQ / OBIC / OBNC** | 9 | These re‑test earlier modules and expect the compiler to *flag* obsolete COBOL‑85 elements. Their language content is covered by the in‑scope specs; obsolete‑feature **flagging** is what is out of scope. |
| **EXEC85** | 1 | Not a test. It is NIST's own COBOL executive that splits the distribution and drives the suite — replaced here by a Rust harness, so it does not need to compile. |

**Object‑Oriented COBOL** is also outside RustCOBOL's scope, but CCVS85 predates
it entirely — there are no OO programs in the suite.

### Where the remaining 192 failures come from

Each is a specified defect, not an unknown. Ranked by the number of programs
whose *first* error it is:

| Programs | Root cause | Spec |
|---:|---|---|
| 31 | separator comma — `MOVE ZERO TO A, B, C` | [separators](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [intrinsics](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [statement gaps](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | space‑separated subscripts — `TBL (1  2)` | [separators](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON` (switch names) and `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md), [separators](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [statement gaps](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | `COPY` deep in Area B or split across lines | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | separator semicolon — `START F ; INVALID KEY` | [separators](../specs/nist/NIST-spec-separators.md) |
| 4 | `OCCURS` integer on a following line | [separators](../specs/nist/NIST-spec-separators.md) |
| 4 | `SECTION` with a priority number — `SORT-PARA SECTION 69.` | [segmentation](../specs/nist/NIST-spec-segmentation.md) |

> **The ranking moves after every fix, and the moves are informative.** Three
> rows that led this table in earlier releases are gone — IDENTIFICATION
> comment-entries, numeric literals, and the stray quotation mark. Each time,
> most of the programs in the cleared row did **not** start passing; they moved
> to the row below. The four SG programs freed at 1.62.12 now stop at
> `SORT-PARA SECTION 69.`, which is why Segmentation still reads 0 / 13.
> Re-measure rather than trusting a previous ranking.

### Conformance history

| Version | PASS / 434 | What changed |
|---|---:|---|
| 1.62.7 | **0** | Nothing compiled. Two rules of the classic reference format were missing: columns 73‑80 were read as source, and continuation lines were never joined. |
| 1.62.8 | **222** | `--source-format=fixed` — the classic reference format, including continuation. See [Source formats](#source-formats). |
| 1.62.10 | **237** | Numeric literals may begin with a decimal point (`.999`). Intrinsic functions 21 → 29, Nucleus 25 → 29, Sort/Merge 27 → 30. |
| 1.62.11 | 241 | IDENTIFICATION comment-entry paragraphs. Debug 5 → 9. A smaller gain than the 32-program bucket suggests: 9 of those are Communication programs (N/A), and most of the rest hit a second blocker immediately after. |
| 1.62.12 | 242 | A literal is confined to its line, so one stray quotation mark can no longer shift the parity of a whole file. Nucleus 29 → 30. The 6‑program bucket cleared: 4 moved on to segment priority numbers, 1 now passes. |
| 1.62.13 | 292 | Separator comma and semicolon are punctuation, not tokens; subscripts may be separated by spaces alone; a subscript may follow a complete qualified name; a doubled delimiter inside a literal is one character. Nucleus 30 → 56, Inter-program 32 → 44, Indexed 31 → 38. Three whole diagnostic buckets emptied. |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — a whole table as an intrinsic argument; `MOVE ALL "X"` fills the field; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`; a signed literal as a `WHEN` object; `PERFORM … TIMES` with a data-item count; an integer count written on a continuation line. **Intrinsic functions 45 / 45 — module complete.** |
| 1.62.15 | 332 | An unknown `FUNCTION` name is a compile error instead of returning 0; a user-defined word may begin with a digit (`25COUNT`, `3-DEM-TBL`, `0 SECTION.`); a `D` line is a comment unless `WITH DEBUGGING MODE`. Segmentation 0 → 10, Nucleus 58 → 61. |
| 1.62.16 | 376 | The `AT` in `AT END` is optional, so a bare `END` phrase no longer swallows the next paragraph header (33 programs). The COPY/REPLACE preprocessor confines a literal to its line, so the word COPY in the copyright banner is not a directive. A numeric literal may open an `ADD`/`SUBTRACT` operand list with its decimal point. **Indexed I/O complete, 42 / 42.** |
| 1.62.17 | 380 | `LINAGE` page layout, `LINAGE-COUNTER`, and `WRITE … AT END-OF-PAGE` / `AT EOP` — implemented, not stubbed. Sequential I/O 77 → 81. |
| **1.62.19** | **396** | A numeric-edited item is a numeric item. The editing decimal point keeps a digit that follows it (`PIC ZZ,ZZZ.9` no longer truncates to `ZZ,ZZZ`), and a picture built only from editing characters — `ZZZZ`, `$.**`, `$**.**CR` — is numeric-edited rather than alphanumeric. Both made a legal arithmetic `GIVING` receiver look non-numeric. |
| **1.62.18** | **391** | A number opening a continuation line is an operand where an expression is expected. The `IS` is optional in a class or sign condition, and a condition may be an `EVALUATE` subject. A procedure-name may be written entirely in digits, in references and headers alike. |
| **1.62.21** | **417** | The Nucleus pass. `ALTER` is a series and `GO TO.` is the altered GO TO; an all-digit procedure-name keeps its leading zeros; a condition-name may be subscripted or qualified; a parenthesised arithmetic expression is an operand, not a nested condition; `MULTIPLY`/`DIVIDE` format 1 take a receiver series; `WITH TEST` may precede `VARYING` and a repeat count may be subscripted; `PERFORM imperative … END-PERFORM` needs no phrase; a paragraph-name may be qualified by its section; `ELSE` is not swallowed by an `ON SIZE ERROR` imperative or a nested ELSE branch; abbreviated combined relations accept arithmetic and class/sign objects; `INSPECT` carries its ALL/LEADING category across operands and `CONVERTING` takes a region; `UNSTRING TALLYING` follows `WITH POINTER`. **Nucleus 76 → 92 of 95 compiling, 16 → 28 executing clean.** |

> **The honest summary.** RustCOBOL accepts just over **three quarters** of the in‑scope
> NIST suite today, up from none seven releases ago. The remaining 102 are not
> mysterious — they are ten named defects, each specified with the programs it
> blocks. This table is the measure of progress, and it is updated with every
> release.

---

> **Update (gap-implementation pass):** the following were implemented and are
> now ✅ — **reference modification** `id(start:len)`, **inline
> `PERFORM n TIMES`**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING `ON OVERFLOW` +
> `END-STRING`/`END-UNSTRING`**, **category-aware `INITIALIZE`**, **operator-
> prefixed abbreviated conditions** (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`**
> (runs on unresolved CALL), **`COMPUTE` multiple receivers + per-receiver
> `ROUNDED`**, and a much larger **intrinsic-function set**.
>
> **Update (hierarchical / occurrence-aware environment pass — 1.5.0):** four
> data-model-blocked features are now ✅ — **runtime table subscripting** `t(i)`
> / `t(i, j)` (per-occurrence storage), **qualified-name disambiguation**
> `id OF/IN group` (duplicated leaf names resolve to independent storage),
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**, and **functional `SEARCH` / `SEARCH ALL`**.
>
> **Update (verb-completeness pass — 1.6.0):** now also ✅ — **multi-receiver
> `MULTIPLY`/`DIVIDE GIVING` + per-receiver `ROUNDED`** on `ADD`/`SUBTRACT`;
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** and corrected
> plain `EXIT`; **`CALL … NOT ON EXCEPTION`**; **`INSPECT … TALLYING …
> REPLACING`** combined and **`BEFORE/AFTER INITIAL`** regions; date/financial
> **intrinsics** (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`,
> `DAY-OF-INTEGER`, `ANNUITY`, `FRACTION-PART`); **literal-object abbreviated
> conditions** (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-subject) and
> **`WHEN NOT`**; **real 88-level condition-names** (`SET … TO TRUE/FALSE`, host
> tested against its VALUEs/ranges); **`PERFORM para VARYING`**; and a functional
> **`SORT`/`MERGE`** runtime (`RELEASE`/`RETURN`, `USING`/`GIVING`, `INPUT`/`OUTPUT
> PROCEDURE`). The avoid-list at the bottom is current.
>
> **Update (avoid-list clearance pass — 1.7.0):** the remaining gaps are now
> implemented — **identifier-object abbreviation** (`a = b OR c`, resolved via
> 88-level metadata); **`INITIALIZE … REPLACING category DATA BY value`**;
> **`66 RENAMES`** (read synthesizes / write distributes across covered items);
> **pointers** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> `SET ADDRESS OF item TO …` aliasing, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; faithful **`NEXT SENTENCE`**; the remaining standard
> **intrinsics** (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`,
> `TEST-NUMVAL`); and extended **screen `ACCEPT`/`DISPLAY`** (`AT`/`WITH` via
> ANSI in CLI mode — now *executed*, not just parsed).
>
> **Update (1.7.1):** the `ACCEPT` register sources are now functional (were
> recognized no-ops) — **`FROM COMMAND-LINE`**, **`ARGUMENT-NUMBER`** /
> **`ARGUMENT-VALUE`** (paired with `DISPLAY n UPON ARGUMENT-NUMBER`),
> **`ENVIRONMENT-VALUE`** (paired with `DISPLAY "name" UPON ENVIRONMENT-NAME`),
> **`ESCAPE KEY`** → `"00"`, **`CRT STATUS`** → `"0000"`.
>
> **Update (1.7.2):** file-sharing / locking phrases and `CANCEL` (were ❌ /
> no-op) — **`OPEN … SHARING WITH … [WITH LOCK]`**, **`READ … WITH [NO] LOCK`**,
> **`UNLOCK`** (releases the file's INDEXED record locks), and **`CANCEL program`**
> (re-initialises the program's storage).
>
> **Update (1.8.0):** **`COMMIT` / `ROLLBACK`** are now real COBOL verbs —
> program-controlled transactions over the open INDEXED files (both the memory
> and disk engines). The disk engine gained a real in-run undo log (it was a
> no-op before). The avoid-list at the bottom is current.

---

## IDENTIFICATION DIVISION paragraphs

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ The **comment‑entry** paragraphs — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — in **any order and any subset**.
- ✅ `REMARKS` is accepted too. It was deleted from COBOL in 1985, so it is not
  stored; it is taken so that source carried over from COBOL‑74 still compiles.

A **comment‑entry** is free text, and COBOL‑85 means that literally:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- It may contain **reserved words** — the `DATA` above does not start a DATA
  DIVISION.
- It may contain **periods**, and does not end at one.
- It **spans as many lines** as you write.
- It ends at the next paragraph or division header **beginning a line** in
  Area A — which is how the entry above ends at `DATE-WRITTEN`.

**A quotation mark in that prose is contained to its line** (since 1.62.12).
Text such as `THE COMPILER"S ABILITY` no longer opens a literal that runs into
the rest of the program — see [Source formats](#source-formats). It is still
worth avoiding an unpaired quote in a comment‑entry, but it now costs you that
line, not the file.

⚠️ `INSTALLATION`, `SECURITY` and `REMARKS` are **not reserved words** here.
They are recognised as paragraph names only inside the IDENTIFICATION DIVISION,
so a data item called `SECURITY` keeps working.

---

## Source formats

RustCOBOL reads three source layouts. The choice is explicit — it is **never**
guessed from the file's contents, because applying column rules to source that
was not written for them deletes code silently.

| `--source-format` | What it means |
|---|---|
| `free` | No column rules at all. `*>` starts a comment. **The default**, and what PowerRustCOBOL's own projects and generated form `.cbl` files use. |
| `fixed` | ✅ **Classic COBOL-85 reference format** — the layout the standard defines and that card-image source is written in. See below. |
| `fixed-relaxed` | The sequence area and indicator column are honoured, but the line runs as far as you typed it — no 72-column limit. |
| `auto` | Historical behaviour: `free`, unless `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` sets the default for a session.

### `fixed` — the classic reference format

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Columns 1-6** — sequence number area, ignored.
- **Column 7** — indicator area:
  - `*` or `/` → comment line
  - `-` → **continuation** of the previous line
  - `D` → debugging line; a comment (debugging mode is not yet implemented)
  - anything else → read as ordinary source. The standard reserves this column,
    but card-image suites use it as a selector for optional lines, and silently
    dropping those lines would delete code.
- **Columns 8-72** — the source.
- **Columns 73-80** — identification area, **discarded**.

### Continuation lines ✅

A hyphen in column 7 continues the previous line.

**Continuing a word or a numeric literal** — the continued line's trailing
spaces are discarded and the halves meet with nothing between them:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

declares one item named `WRK-DS-18V00-CONTINUED`.

**Continuing an alphanumeric literal** — the continued line's literal has no
closing quotation mark; the continuation line must reopen with one, and the
literal resumes at the character after it:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **The continued fragment runs to column 72, trailing spaces included.** A line
that stops short of column 72 still contributes those spaces to the literal.
This is why a continued literal is only byte-exact under `fixed`; the other
formats have no column 72 to stop at.

### A literal never spans a line by accident ✅

Continuation is the **only** way a literal reaches across lines. A quotation
mark that is not closed on its own line is an error, reported where it is
written:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

This matters more than it sounds. Before 1.62.12 an unpaired quote ran to the
*next* quotation mark anywhere in the file, so a single stray `"` in a comment
swallowed whole divisions and shifted the pairing of every quote after it — the
NIST programs where this was found have an **even** number of quotation marks,
so nothing was unterminated; one character had shifted the parity of the entire
file. The damage now stops at the newline.

> **Free format has no literal continuation.** Not `&` — that is the
> concatenation *operator* — and not a fenced block. A free-format literal must
> fit on one line; for a long one, concatenate: `"first part" & "second part"`.

> **Note.** Choosing `fixed` for a file that was written free-form will damage
> it — anything past column 72 vanishes, and text before column 8 is read as a
> sequence number. Only pass it for source that really is card-image.

---

## Recognized statements (verbs)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirects para-1's `GO TO`) ·
`UNLOCK file` (releases the file's record locks) · `OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (file sharing/locking — advisory in the single run unit)
✅ `COMMIT` / `ROLLBACK` (program-controlled INDEXED-file transactions — see
File verbs) · `CANCEL` (re‑initialises the program's storage) ·
⚠️ `INVOKE` (parsed as no‑op)
Project extensions: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`, `THROW`.
A block may `use` the always-linked crates (std, egui, eframe and the linked
runtime set) **plus any crate the project registers under Project's Crates**
(spec 044): registered crates are pinned to an exact version, vendored in the
project's `crates/`, and compiled into the binary; unregistered crates fail
Check/Build at the developer's line with the remedy named.

✅ `SEARCH` (serial) / `SEARCH ALL` (binary search over an `ASCENDING`/
`DESCENDING KEY` table — runs the first matching `WHEN`, else `AT END`).
✅ `SORT` / `MERGE` with `RELEASE` / `RETURN` (functional — see below).
✅ `DECLARATIVES … END DECLARATIVES` with `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — file-error handlers fired on an
unhandled error `FILE STATUS`.
❌ **Not recognized — do not use:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Per‑verb supported forms

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (multiple receivers).
- ✅ `MOVE CORRESPONDING g1 TO g2` — moves each subordinate item the two groups
  share by name, recursing through matching sub-groups.
- ✅ **Reference modification `id(start:len)`** — sender (substring) and receiver
  (spliced partial assignment); works on every verb's operands. `length` optional.
  It addresses **character positions**, so a numeric operand is taken at its full
  `PIC` width with its leading zeros: `01 T PIC 9(8) VALUE 00224845` gives
  `T(1:2)` = `"00"`, not `"22"`.
- ✅ **Group items are alphanumeric aggregates** — a group *is* its subordinate
  items laid end to end, and its size is the sum of theirs. Reading one
  concatenates the children (including `FILLER`); moving to one distributes the
  bytes across them by width. `MOVE 11 TO A` is visible through the group that
  contains `A`, and `MOVE "1234" TO G` sets `G`'s children, not a slot of its own.
- ✅ subscripts `t(i)`, `t(i, j)` — read/write the per-occurrence storage slot;
  variable subscripts `t(WS-I)` evaluated each access.
- ✅ qualification `id OF/IN group` (`… OF g1 OF g2`) — resolves to the correct
  item even when the leaf name is declared under more than one group.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **per‑receiver `ROUNDED`** — each receiver carries its own `ROUNDED` flag.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine each matching numeric
  pair, recursing through matching sub-groups.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **multiple `GIVING` receivers**, each with its own `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (no `GIVING`) stores `a/b` back into `a` (a PowerRustCOBOL
  convenience; standard COBOL requires `INTO` or `GIVING` here).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **multiple receivers, each with its own `ROUNDED`**.
- ✅ expr operators `+ - * /` and `**` (power, right‑assoc), parentheses,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **`ALSO` multi‑subject** — each `WHEN` column is matched positionally
  against its subject and AND‑combined.
- ✅ **`WHEN NOT value`** negates a selection object; **`WHEN condition`**
  (e.g. `EVALUATE TRUE WHEN a > b`) evaluates the boolean condition.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = integer literal or data‑item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ inline `PERFORM UNTIL cond … END-PERFORM`,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ inline `PERFORM n TIMES … END-PERFORM` (no paragraph).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — runs the paragraph each
  iteration (out‑of‑line, no `END-PERFORM`).

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ plain `EXIT` is a no‑op return point; `EXIT PROGRAM` returns to the caller.
- ✅ `EXIT PERFORM [CYCLE]` (break / continue the nearest inline PERFORM),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfers control past the next sentence boundary (the
  parser inserts boundary markers at each period; faithful, not just `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` positions the cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (whole command line) · `FROM ARGUMENT-NUMBER` (arg count)
  · `FROM ARGUMENT-VALUE` (arg at the pointer set by `DISPLAY n UPON
  ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE` (the
  variable named by `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` closes the statement (optional).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` closes the operand list (optional), so
  `DISPLAY A END-DISPLAY DISPLAY B` is two statements rather than one.
- ✅ screen forms `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — executed via ANSI cursor
  positioning + SGR in **CLI mode** (`rcrun`); ignored in GUI mode (the form
  designer supersedes SCREEN I/O there). `ACCEPT id AT …` positions then reads.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Overflow = the assembled string is wider than the receiving field.
- ✅ **Extension — smart default `DELIMITED BY`** (when the clause is omitted on
  an operand): alphanumeric `PIC X`/`A` items default to `SPACES` (trailing pad
  dropped); string literals, numeric, numeric-edited items, `FUNCTION` results
  and expressions default to `SIZE`. Data items are moved in their field form
  (numeric → full PIC-width digits; numeric-edited → edited characters).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Overflow = more source fields than
  receivers.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **both halves applied**.
- ✅ `BEFORE/AFTER INITIAL` confines each phrase to a sub‑region of the field.
  (TALLYING accumulates onto the counter, per COBOL.)

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compiled to MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encoded as ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` sets the host item to the condition's first VALUE;
  `TO FALSE` sets a value outside the VALUE set (best effort — no FALSE clause).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` and
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — see **Pointers** below.

### INITIALIZE
- ✅ `INITIALIZE id …` — category-aware: numeric / numeric-edited → ZERO,
  everything else → SPACES, recursing into group items.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — sets each
  subordinate item of that category to the value; others untouched.

### Pointers (USAGE POINTER)
- ✅ `USAGE POINTER` declares a pointer (NULL initially).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — aliases `id` onto the
  target's storage (reads **and** writes follow the alias); typically a LINKAGE
  record. `IF ptr = NULL` works.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ The `ON EXCEPTION` / `ON OVERFLOW` body runs when the called program is
  unresolved; the `NOT ON EXCEPTION` body runs when the call **resolves**.
- ✅ `CANCEL program …` re-initialises the named program's WORKING-STORAGE so its
  next `CALL` starts fresh.

### File verbs (the supported phrases — full coverage is in the file‑I/O suite)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` parse and are honoured where meaningful — advisory in
  the single‑run‑unit model.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (PowerRustCOBOL
  extension) — records the operator/user in the INDEXED observability log
  (`user=` field on every event line for that file's session). Purely
  observational; no authentication/authorization. See
  [`observability-en.md`](observability-en.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` releases the record lock the INDEXED engine takes under I‑O.
- ✅ `UNLOCK f [RECORD[S]]` releases the file's record locks.
- ✅ **`COMMIT` / `ROLLBACK`** — program-controlled transactions over **every**
  open INDEXED file. `OPEN` starts a transaction; `COMMIT` confirms the pending
  `WRITE`/`REWRITE`/`DELETE`s (a later `ROLLBACK` can no longer undo them) and
  starts a new one; `ROLLBACK` undoes every change since the last `COMMIT`/`OPEN`.
  **DISK** storage makes `COMMIT`/`CLOSE` durable on disk. **MEMORY** storage
  keeps `COMMIT`/`ROLLBACK` purely in RAM (never writes to disk); a plain
  `STORAGE IS MEMORY` file is ephemeral, and `STORAGE IS MEMORY WITH PERSISTENCE`
  saves to disk on `CLOSE` only. (Crash-recovery via a durable write-ahead log is
  future work — this is in-run, program-level rollback.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (INDEXED files; PowerRustCOBOL extension). Default storage is
  `DISK`. `WITH COMPRESSION` compresses the stored record (keys evaluated on the
  uncompressed record); `WITH PERSISTENCE` (MEMORY only) saves the in-RAM file on
  `CLOSE`. `OPEN OUTPUT` always (re)creates the on-disk container.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ Cross‑*process* file sharing is not enforced (single run unit); the
  `SHARING`/`LOCK` phrases parse and the INDEXED engine's per‑run record locks
  are honoured.

### SORT / MERGE / RELEASE / RETURN  ✅ (functional, in‑memory work buffer)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (in an INPUT PROCEDURE) appends to the run;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` hands records back.
- Records are stable‑sorted by the declared keys (`ASCENDING`/`DESCENDING`);
  `USING` reads / `GIVING` writes the named sequential files.

---

## Conditions (IF / EVALUATE / PERFORM UNTIL)

- ✅ Relational symbols: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Word relations: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Class: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
- ✅ Sign: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ 88‑level condition‑name (bare name as a condition).
- ✅ **`TRUE` / `FALSE` as operands** (PowerRustCOBOL extension) — sugar for `1`
  and `0`, wherever a value is allowed: `IF x = TRUE`, `IF x IS [NOT] FALSE`,
  `IF x NOT TRUE` (the bare `NOT` form, no relational operator),
  `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`, `COMPUTE n = n + TRUE`,
  `INVOKE obj "m" USING TRUE`, and `WHEN TRUE` against a value subject. A bare
  `TRUE`/`FALSE` is also a complete condition (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ This does **not** change the two places the words already meant something:
  `SET <88‑name> TO TRUE` still sets the host item to a value satisfying the
  condition (not the number 1), and `EVALUATE TRUE`/`EVALUATE FALSE` below
  remain the standard case statement.
- ✅ Combined `AND` / `OR` / `NOT`, parentheses (AND binds tighter than OR).
- ✅ **Operator‑prefixed abbreviated conditions** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (the preceding comparison subject is reused).
- ✅ **Literal‑object abbreviation** — `a = 1 OR 2 OR 3` (reuses both the subject
  and the operator; the object is a literal).
- ✅ **Identifier‑object abbreviation** — `a = b OR c` (where `c` is a data‑item).
  A bare identifier after AND/OR following a comparison is resolved at runtime:
  a known 88‑level condition‑name evaluates as one, otherwise it is the object
  `a = c`. (An identifier immediately followed by `AND` keeps AND precedence.)

---

## Expressions, literals, USAGE

- ✅ Arithmetic operators `+ - * /` and `**`; parentheses; unary `+`/`-`.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — **implemented** intrinsics:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (with optional seed), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Date conversions use the standard base 1601‑01‑01 = day 1.) The **complete
  COBOL‑85 standard intrinsic set** is implemented.
- ✅ **The date and time registers read the LOCAL clock.** `ACCEPT … FROM DATE /
  TIME / DAY / DAY-OF-WEEK` and `FUNCTION CURRENT-DATE` all report the machine's
  own time of day, not UTC — including the date, which differs either side of
  midnight. `CURRENT-DATE`'s last five characters carry the **real** offset from
  GMT (`…-0300`), so a program can tell which zone it is running in.
  ⚠️ Any unrecognised `FUNCTION` name still parses but returns **0** at runtime.
- ✅ Literals: integer, decimal, string, all figurative constants
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **A numeric literal may begin with the decimal point** — `.5`, `-.5`,
  `.000000001`. COBOL‑85 requires only that a literal not *end* with one, so
  `5.` is still the number 5 followed by a sentence terminator.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Leading zeros are significant and exact: `.000000001` is one billionth, not
  one tenth. Under `DECIMAL-POINT IS COMMA` the same applies to `,5`.
  What separates the literal from a sentence-ending period is the **absence of
  a space** — COBOL‑85 requires one after a terminator, so `MOVE X TO Y.` is
  never read as the start of a fraction, and `MOVE X TO Y.5` is a compile
  error rather than a silent reinterpretation.
- ✅ **Hexadecimal literals** — `X"09"`, `x'0D0A'` (either case, either quote).
  One character per **pair** of hex digits, so the digit count must be even; an
  odd count or a non-hex digit is a malformed literal and is reported, not
  quietly re-read as the word `X` beside a string. Usable anywhere a quoted
  literal is (`DELIMITED BY`, `MOVE`, `VALUE`, comparisons).

---

## DATA DIVISION clauses (declaration syntax accepted)

- ✅ Levels `01`–`49`, `77`, `88`; `FILLER`; group/elementary. The word `FILLER`
  is **optional** — `05 PIC X VALUE ":".` declares one just as `05 FILLER PIC X
  VALUE ":".` does, and either way it holds its bytes and its `VALUE` inside the
  group that contains it.
- ✅ `PIC/PICTURE` with `X A 9 S V P` and edited symbols (`Z * $ + - CR DB B 0 /
  , .`). **`P` is a decimal scaling position** — a digit position the item spans
  but does not store: `PIC S999PP` holds three digits standing for hundreds
  (`MOVE 12300` stores it exactly; `MOVE 12345` stores 12300), and `PIC PP99`
  holds two standing for ten‑thousandths. The positions the `P`s occupy always
  read back as zero and take **no bytes** in a record layout.
- ✅ **High‑order truncation on a numeric MOVE.** A receiver holds exactly its
  declared digits at both ends: `01 M PIC 99V999.  MOVE 123.45 TO M.` leaves
  `23.450`. Arithmetic tests the receiver's capacity first, so a statement with
  `ON SIZE ERROR` keeps its old value instead.
- ✅ **A table of groups is addressed per occurrence.** `MOVE VALUES-1 TO
  GRP-1 (2)` distributes across that occurrence's own children
  (`ELEM1 (2,1) … ELEM1 (2,4)`), and reading `GRP-1 (2)` concatenates exactly
  those. The enclosing `01` record is the bytes of **every** occurrence, so
  `MOVE GRP-TAB1 TO GRP-TAB2` copies a whole table.
- ✅ **Index‑names, literals and relative indexing mix as subscripts.**
  `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — a sign glued to its
  digits is a signed literal opening the next subscript — and
  `ELEM1 (IN1 - 1, 3)`, where the operator is spaced on both sides, is relative
  indexing.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (and `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numeric/signed/alphanumeric/figurative/`ALL`).
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — a **live** second reading of the same bytes. It adds no
  storage (so it does not widen the group that holds it), and a write through
  either description is visible through the other:
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` then reads back through `RESULT-A`.
  ⚠️ **Caveat:** an overlay larger than 256 expanded storage slots (a redefined
  10×10×10 table, say) keeps per‑description storage instead — refreshing it on
  every write would walk a thousand occurrences twice.
- ✅ `JUSTIFIED [RIGHT]`, `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — accepted;
  `JUSTIFIED` and `SIGN … SEPARATE` do not yet change how the item is stored.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **real condition‑names**: the
  level‑88 binds to its host item; testing checks the host against the VALUEs /
  ranges, and `SET 88-name TO TRUE` stores a satisfying value into the host.
- ✅ `USAGE INDEX` declares an integer index register (`SET`/`SEARCH` use it);
  `USAGE POINTER` — see **Pointers** above.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — a regrouping alias;
  reading concatenates the covered items, writing distributes by field width.
- Sections: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN`
  parsed but not executed.

---

## Still NOT supported — current avoid‑list

> **Corrected 2026‑08‑25.** This section used to open "The COBOL‑85 verb /
> clause set is **fully covered**." Running the NIST CCVS85 suite disproved it:
> **102 of the 434 in-scope programs failed that day**, on constructs this
> document did not list as gaps — separator commas and semicolons, `FUNCTION x(ALL)`,
> `CLOSE … WITH LOCK`, `COPY` in Area B, IDENTIFICATION comment entries,
> section priority numbers, digit‑leading data names, and — until 1.62.10 —
> numeric literals with a leading decimal point. That is what a validation
> suite is for. Each gap is
> now specified in [`specs/nist/`](../specs/nist/README.md) and tracked in the
> [scoreboard](#-conformance-is-measured-not-asserted--nist-ccvs85) above.

The list below is what is out of scope **by intent**, as opposed to the NIST
gaps above, which are defects being worked through:

1. **Screen `ACCEPT` input editing** — `DISPLAY … AT/WITH` and `ACCEPT … AT`
   are executed (ANSI) in CLI mode, but full field‑level SCREEN SECTION editing
   (auto‑tab, field validation, colour maps) is **superseded by the form
   designer** in GUI mode.
2. **Cross‑*process* file sharing** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK`, and `UNLOCK` parse and drive the INDEXED engine's
   per‑run record locks, but locks are not enforced across separate OS processes
   (single run‑unit model).
3. **Object‑Oriented COBOL** (class/method definitions) — `INVOKE` is a no‑op
   for COBOL objects (it drives GUI/runtime objects only).
4. ⚠️ **RELATIVE** file organization (SEQUENTIAL / LINE SEQUENTIAL / INDEXED
   done). **This one is a trap, not a clean gap:** `ORGANIZATION IS RELATIVE`
   *parses*, and nothing in the runtime ever dispatches on it — so a RELATIVE
   program compiles and then misbehaves with no diagnostic. 30 of the NIST RL
   module's 35 programs are in exactly that state. Treat it as unimplemented.
   Spec: [relative‑organization](../specs/nist/NIST-spec-relative-organization.md).
5. Unrecognised intrinsic‑function names still return **0** — the same silent
   failure mode. Spec:
   [intrinsics](../specs/nist/NIST-spec-intrinsic-function-gaps.md).
6. ⚠️ **An invalid `ACCESS MODE` / `ORGANIZATION` value is swallowed without a
   diagnostic** — the same trap again, and this one is triggered by an ordinary
   user typo. `ACCESS MODE IS` accepts only `SEQUENTIAL`, `RANDOM` or `DYNAMIC`
   (`INDEXED` is an *organization*, not an access mode), but the SELECT clause
   parser tests those three and lets anything else fall through to the generic
   "skip an unknown token" arm, so the file silently keeps the default
   `SEQUENTIAL` and misbehaves at run time instead of failing to compile.
   `ORGANIZATION IS` has the identical shape. Both should raise a clear
   compile‑time error naming the offending word. **Not a Nucleus problem** —
   no NC program carries an `ACCESS MODE` clause; the clause appears only in
   the DB, IC, IX, OBSQ, RL, RW, SQ and ST modules, so under GOLDEN RULE #9
   this waits until NC is finished.
7. ⚠️ **`ALPHABET … IS EBCDIC` is accepted but leaves native (ASCII) ordering
   in force.** The literal phrase (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` and `STANDARD‑2` are all implemented and drive
   `PROGRAM COLLATING SEQUENCE` for real; only the EBCDIC table is missing, and
   naming it silently gets ASCII order. Same trap family as 4–6.
8. **The Communication module and Report Writer** — see
   [N/A above](#-na--what-is-out-of-rustcobols-scope-and-why).

> **Resolved (1.5.0):** the flat data model became hierarchical / occurrence‑aware,
> unblocking **CORRESPONDING**, **qualified names**, **table subscripting**, and
> **`SEARCH`**.
> **Resolved (1.6.0):** multi‑receiver `MULTIPLY`/`DIVIDE` + per‑receiver
> `ROUNDED`; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`; combined
> `INSPECT TALLYING REPLACING` + `BEFORE/AFTER INITIAL`; date/`ANNUITY`
> intrinsics; literal‑object abbreviation; `EVALUATE ALSO`/`WHEN NOT`; real
> 88‑level condition‑names; `PERFORM para VARYING`; and the `SORT`/`MERGE`
> runtime with `RELEASE`/`RETURN`.
> **Resolved (1.7.0):** identifier‑object abbreviation; `INITIALIZE … REPLACING`;
> `66 RENAMES`; pointers (`USAGE POINTER`, `SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`); `ALTER` / `UNLOCK`; faithful `NEXT SENTENCE`; the remaining standard
> intrinsics; and extended screen `ACCEPT`/`DISPLAY` (executed in CLI mode).
> **Resolved (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (with the paired
> `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` registers).
> **Resolved (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (releases INDEXED record locks), and `CANCEL program`.
> **Resolved (1.8.0):** `COMMIT` / `ROLLBACK` as program-controlled INDEXED-file
> transactions (memory + disk engines; real undo log on disk).
