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
right and RustCOBOL is wrong**.

The machine-readable ledger is [`NIST/progress.json`](../NIST/progress.json) —
committed, updated after every verified change. The figures below are taken
from it rather than retyped.

### The scoreboard

Measured **2026‑08‑31 at 1.62.132**, on the untouched distribution. The compile
census closed at 1.62.129.

| Axis | Result | Meaning |
|---|---:|---|
| **Compile** | **420 / 420** | every in-scope program is accepted by the front end. FAIL 0. |
| **Execution** | **380 / 380** | every scored program runs and reports **zero failures** in its own CCVS report. |
| **Assertions** | **8 362 PASS / 0 FAIL** | the checks those programs make on themselves. |

Reproduce either axis:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### The two axes are never conflated

Compile is the strictly weaker claim: it says the front end accepts every
construct in a program, not that the program computes the right answer. The
suite scores itself — every CCVS85 program prints its own `PASS` / `FAIL*`
tally — so the execution axis is the one that means "it works". Both are
reported per module below, with their own denominators, and neither is ever
quoted as the other.

The clearest illustration is in this repository's own history: 30 of the 35
RELATIVE‑file programs compiled cleanly while the runtime had **no RELATIVE
engine at all**. They ran and produced wrong results silently. The engine
landed at 1.62.76 and the module finished at 1.62.77.

#### Per module

Compile and execution carry different denominators, for two stated reasons.
The `*301M` members test *intermediate-subset flagging* of features RustCOBOL
implements as standard, which is unreachable by design and excluded from
execution by operator ruling (IX301M, RL301M, ST301M, SM301M); they still count
on the compile census, where they pass. And most IC members are **callees** —
subprograms with no report of their own — so only the calling programs are
scored.

| Module | What it tests | Compile | Execution | Assertions | State |
|---|---|---:|---:|---:|---|
| **NC** | Nucleus | **95 / 95** | **95 / 95** | 4 614 | ✅ finished |
| **SQ** | Sequential I/O | **85 / 85** | **85 / 85** | 624 | ✅ finished |
| **IX** | Indexed I/O | **42 / 42** | **41 / 41** | 574 | ✅ finished |
| **IF** | Intrinsic functions | **45 / 45** | **45 / 45** | 841 | ✅ finished |
| **IC** | Inter‑program communication | **47 / 47** | **25 / 25** | 309 | ✅ finished |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ finished |
| **SM** | Source text manipulation | **17 / 17** | **16 / 16** | 311 | ✅ finished |
| **RL** | Relative I/O | **35 / 35** | **34 / 34** | 354 | ✅ finished |
| **DB** | Debug | **14 / 14** | — | — | compile axis only (below) |
| **In scope** | | **420 / 420** | **380 / 380** | **8 362** | |
| SG | Segmentation | 13 / 13 | — | — | ⬜ ruled out of scope (below) |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB (Debug)** is scored on compile only. Its 14 programs are accepted; the
debug module's *runtime* semantics are not implemented, and the execution axis
for it has not been ruled in. It is listed here rather than hidden, so the gap
stays visible.

#### The DELETED count — 24, and what it means

`***** ****TEST DELETED****` is CCVS's own marker for a case the program itself
skipped. It is **not** a pass, and it is tracked separately for that reason:
the count fell 108 → 1 at 1.62.53 while the clean-program count barely moved,
which was real progress a failures-only reading would have missed.

Across the finished modules 24 cases are DELETED: NC 5, SQ 6, IX 1, IC 4,
SM 3, RL 5. **Only SM's 3 are documented as the distribution's own** — SM206A
PST‑TEST‑008 and PST‑TEST‑11, and SM208A REP‑TEST‑7, ship commented out, so a
conforming run of the shipped source reports exactly those three. The other 21
are recorded but not yet individually explained in the ledger; do not quote
them as by‑design.

### ⬜ N/A — what is out of RustCOBOL's scope, and why

These modules are **not counted as failures** — 38 programs excluded from every
score. Full reasoning in
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Module | Programs | Why it is out of scope |
|---|---:|---|
| **CM** — Communication | 9 | `COMMUNICATION SECTION`, `CD` entries, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Targets 1980s teleprocessing monitors — message queues owned by a transaction manager. There is no such runtime here, and the module was removed from later COBOL standards. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, `RD` entries, `INITIATE` / `GENERATE` / `TERMINATE`, control breaks. A large declarative sub‑language; PowerRustCOBOL's answer to reporting is the Form Designer and PDF export. Could become a *feature* later if wanted — it is the one exclusion with real user value. |
| **SG** — Segmentation | 13 | Operator ruling, 2026‑08‑29. Segmentation exists to fit a program into a machine too small to hold it: `SECTION` headers carry a segment number and the runtime overlays independent segments over one another. RustCOBOL is a 64‑bit runtime with more address space than any COBOL program can exhaust, so a segment number **compiles and has no effect at all**. There is no behaviour for the module to measure. Its 13 programs still compile, and are reported N‑A rather than deleted so the exclusion stays visible. |
| **OBSQ / OBIC / OBNC** | 9 | These re‑test earlier modules and expect the compiler to *flag* obsolete COBOL‑85 elements. Their language content is covered by the in‑scope specs; obsolete‑feature **flagging** is what is out of scope. |
| **EXEC85** | 1 | Not a test. It is NIST's own COBOL executive that splits the distribution and drives the suite — replaced here by a Rust harness, so it does not need to compile. |

**Object‑Oriented COBOL** is also outside RustCOBOL's scope, but CCVS85 predates
it entirely — there are no OO programs in the suite.

### What is left

On the scored modules, nothing: both axes are closed and no assertion fails.
What remains is not a defect list but three standing rulings — DB's execution
axis, SG, and the `*301M` flagging members — each recorded above with its
reason, plus the 21 DELETED cases that have not been individually explained.

The harness prints the failure detail behind any regression, ready to bucket
across a module:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> A `FAIL*` detail line is written **twice** on purpose — CCVS's `PRINT-DETAIL`
> runs `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — while `PASS ` is
> written once. Any raw marker count taken from the print file has to halve the
> failures before it means anything.

### Conformance history

Compile axis, against the in‑scope denominator of the day. The denominator
itself moved when SG was ruled out and when DB205A was re‑scored under CM, so
the early rows are out of 434 and the closing row out of 420.

| Version | Compile | What changed |
|---|---:|---|
| 1.62.7 | **0** / 434 | Nothing compiled. Two rules of the classic reference format were missing: columns 73‑80 were read as source, and continuation lines were never joined. |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — the classic reference format, including continuation. See [Source formats](#source-formats). |
| 1.62.13 | 292 / 434 | Separator comma and semicolon are punctuation, not tokens; subscripts may be separated by spaces alone; a doubled delimiter inside a literal is one character. Three whole diagnostic buckets emptied. |
| 1.62.14 | 317 / 434 | A whole table as an intrinsic argument; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`. **Intrinsic functions 45 / 45 on compile.** |
| 1.62.16 | 376 / 434 | The `AT` in `AT END` is optional, so a bare `END` phrase no longer swallows the next paragraph header (33 programs). **Indexed I/O 42 / 42 on compile.** |
| 1.62.21 | 417 / 434 | The Nucleus pass — `ALTER` series, subscripted condition‑names, abbreviated combined relations, `INSPECT` categories across operands. Nucleus 76 → 92 compiling. |
| **1.62.42** | 420 / 434 | **Nucleus finished on both axes** — 95 / 95 compiling *and* executing clean, 4 614 assertions with none failing. |
| **1.62.43** | 422 / 434 | Sequential I/O compiles completely, 85 / 85, and goes 10 → 44 of 85 on execution. A declarative's paragraphs keep their names, so a `USE` handler can `PERFORM` and `GO TO` them — 20 programs stopped crashing. |
| **1.62.47** | — | **Sequential I/O finished** — 85 / 85 on both axes. The last gap was `XXXXD001`, a data file the CCVS85 *installation* supplies and no member writes; the harness now plants it. |
| **1.62.76** | — | The **RELATIVE engine** lands (`cobolt-runtime/src/relative.rs`, container `PRCREL1`). All seven file verbs dispatch on `FileOrganization::Relative`. |
| **1.62.77** | — | **Relative I/O finished** (34 / 34) from a 14 / 35 baseline in one session, and **Indexed I/O finished** — the relative engine closed IX106A's last four failures, which were the relative file it exercises alongside the sequential and indexed ones. |
| **1.62.81** | — | **Intrinsic functions finished** on execution, 45 / 45, from a 24 / 45 baseline. Five causes, none of them in the module's own subject matter: a lexer separator rule twice, a flagging gap, `NUMVAL`'s argument grammar, an argument‑list comparison, and a division overflow path. |
| **1.62.107** | — | **Inter‑program communication finished**, 25 / 25. |
| **1.62.119** | — | **Sort / Merge finished**, 39 / 39, 735 PASS. The final step was `[COLLATING] SEQUENCE [IS] alphabet-name` on SORT/MERGE, ordering alphanumeric keys by the named `SPECIAL-NAMES` alphabet. |
| **1.62.127** | — | **Source text manipulation finished**, 16 / 16. String‑literal operands keep their quotes; identifier operands span their `IN`/`OF` chain and subscript; pairs apply in one pass with no rescan of replacements. |
| **1.62.129** | **420 / 420** | **The compile census closes at 100 %.** DB205A is scored under CM by ruling, which puts the in‑scope suite at 420. |

> **The honest summary.** Every in‑scope program compiles, and every scored
> program runs clean: **420 / 420 on compile, 380 / 380 on execution, 8 362
> assertions with none failing.** Nine releases before the first of those rows
> the compile figure was zero. What is still open is stated above as rulings
> rather than hidden inside a percentage — DB's execution axis, SG, the `*301M`
> flagging members, and 21 DELETED cases that have not been individually
> explained.

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
unhandled error `FILE STATUS`. A handler is **entered at the top of its section
and runs to the section's end**, and its paragraphs keep their names, so it may
`PERFORM` and `GO TO` them — including a paragraph of *another* declarative
section. Declarative paragraphs live in their own name space: control never
falls from the main body into them, and a name declared in both resolves to the
declarative's copy while a handler is running and to the body's everywhere else.
A declarative may also `PERFORM` a paragraph of the non-declarative portion.
❌ **Not recognized — do not use:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Per‑verb supported forms

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (multiple receivers).
- ✅ **A group operand makes the whole move alphanumeric** (COBOL-85 6.18.4).
  The other operand's PICTURE contributes its *size* and nothing else: no
  editing, no de-editing, no numeric conversion. `MOVE <group holding "123ABC">`
  leaves `"123ABC "` in a `PIC 0XXXXX0` (not the edited `"0123AB0"`), the same
  six characters and a space in a `PIC 9999V999`, and `"12"` in a `PIC 99`.
  `JUSTIFIED RIGHT` still decides which end pads and which end is lost.
  The same rule governs a group's own bytes: each child takes its slice
  verbatim, so an alphanumeric-edited child is **not** re-edited.
- ✅ **A `VALUE` clause on a group** initialises the group's bytes and is
  distributed across its children — `01 G VALUE "$123.45". 02 E PIC $999.99.`
  leaves `E` holding `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — moves each subordinate item the two groups
  share by name, recursing through matching sub-groups.
- ✅ **`CORRESPONDING` excludes an item described with `REDEFINES` or `RENAMES`**
  (COBOL-85 6.18.4 GR1), on either side, along with everything subordinate to
  it. The exclusion is on the *declaration*, not the name: a plain item that
  merely shares its name with a 66-level elsewhere still corresponds.
- ✅ **Either `CORRESPONDING` operand may name one occurrence of a table of
  groups** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` writes that
  occurrence's own slots, and the subscript is carried through the recursion.
- ✅ **A pair needs only ONE of its two items to be elementary.** A group may
  face an elementary item, and the move across it is an alphanumeric one: an
  elementary `PIC XXX` sending into a group of `999` + `XXX` fills its six
  characters, and a group of `XXX` + `99` sending into a plain `X(5)` fills it.
  Two groups facing each other still **recurse** — that pairing is not the
  elementary case. *(Before 1.62.39 either direction moved nothing at all: a
  group owns no store slot, so the write went where nothing reads it back and
  the read yielded the empty string.)*
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
- ✅ **`WITH TEST AFTER` applies to `VARYING`**, written on either side of the
  phrase and inline or out-of-line. The body runs once before anything is
  tested, and the conditions are then tested **innermost first**; the level whose
  condition is false is augmented, every level inside it restarts at its `FROM`
  value, and the body runs again. A variable is augmented only when its test
  comes out false, so the test that ends the loop leaves it as the body did.
- ✅ **An `AFTER` variable is reset to its `FROM` value when its loop ends**,
  before the next level out is augmented (COBOL-85 6.20.4 GR10(d)). After the
  whole `PERFORM`, the inner variables read their `FROM` values and only the
  outermost holds the value that ended it.
- ✅ **A subscripted `VARYING` identifier follows its subscript.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` augments
  whichever occurrence `S1` selects at that moment, so a body that advances `S1`
  walks the table.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **The `{OF|IN} section` qualifier picks which copy is meant** when a
  paragraph name repeats across sections, exactly as it does on `PERFORM`. An
  **unknown** section falls back to the unqualified lookup rather than losing
  the jump. `GO TO … DEPENDING ON` takes a bare list of names and no qualifier,
  and a `GO TO` an `ALTER` has redirected follows the redirection — which names
  its own target outright. *(Before 1.62.39 the qualifier parsed and was then
  ignored, so the jump landed on the first definition anywhere in the program.)*
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
- ✅ **`FROM mnemonic-name` reads the operator** when `SPECIAL-NAMES` declares
  the mnemonic (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — that is Format 1, identical to a bare `ACCEPT id`.
  A name **no `SPECIAL-NAMES` clause declares** keeps the PowerRustCOBOL
  extension and reads the **environment variable** of that name. Which of the
  two applies is decided by the declaration, never by the spelling.
  *(Before 1.62.35 the ordinary `<implementor-name> IS <mnemonic>` clause was
  skipped outright, so every mnemonic read an environment variable that was
  never set and the receiving item was left empty.)*
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
- ✅ **A `DELIMITED BY` phrase governs the whole series of senders that precedes
  it**, not only the one it is written after:
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` delimits all three and
  builds `"ABC"`. A statement may carry several phrases, each governing the
  senders since the previous one; senders after the last phrase take the whole
  of each. *(Before 1.62.40 only the sender written immediately before the
  phrase was delimited.)*
- ✅ **`INTO` a group item** distributes across the group's subordinate items.
- ✅ **The result is assembled byte for byte**, so `STRING HIGH-VALUE` moves the
  single byte `0xFF` and occupies one character position.
- ✅ **Extension — smart default `DELIMITED BY`** (when no phrase governs an
  operand): alphanumeric `PIC X`/`A` items default to `SPACES` (trailing pad
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
- ✅ **A series of TALLYING operands shares ONE left-to-right scan** (COBOL-85
  6.17.3). At each character position the operands are tried in the order they
  were written; the first that matches takes the position and the scan resumes
  past the characters it consumed. So `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"`
  on `"AABA"` gives `t1 = 1, t2 = 1` — writing the operands the other way round
  gives `t1 = 3, t2 = 0`. `LEADING` must match from its window's left edge with
  no gap, so an earlier operand taking that position ends the run before it
  starts, and `CHARACTERS` counts only the positions no earlier operand claimed.
- ✅ **A series of REPLACING operands shares ONE scan too**, by the same rule:
  the first operand that matches at a position replaces those characters and the
  scan resumes past them, so no later operand can see them. Each operand's
  `BEFORE`/`AFTER` window is fixed **before any replacement**, which is what
  lets one operand be anchored on characters an earlier one overwrites:

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Applied one operand at a time the first phrase would erase the `"L "` the
  second is anchored on, and `"BAD"` would survive.
- ✅ **A signed DISPLAY item has no `-` among its character positions.** The
  operational sign is an overpunch on a digit, so
  `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` gives **0** while
  `FOR ALL "5"` gives 1. The sign is restored afterwards, so a `REPLACING` over
  the digits leaves it alone. `SIGN IS … SEPARATE CHARACTER` is the case where
  the sign *is* a position, and it is counted.

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
- ✅ **One `OPEN` may carry several mode groups**, each with its own files:
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Every group is opened in its own
  mode; `SHARING` / `WITH LOCK` / `REGISTERED USER` apply to the statement.
- ✅ **`OPEN` of a file that is already open is `41`**, and the file is left as
  it was — the statement does **not** re-open it. (Re-opening an `OUTPUT` file
  would silently truncate what the program had already written.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (PowerRustCOBOL
  extension) — records the operator/user in the INDEXED observability log
  (`user=` field on every event line for that file's session). Purely
  observational; no authentication/authorization. See
  [`observability-en.md`](observability-en.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` releases the record lock the INDEXED engine takes under I‑O.
- ✅ **`READ … INTO id` is the `READ` followed by a group `MOVE`.** The record is
  distributed across the receiver's subordinate items by width and cut at the
  receiver's own width, the receiver may be subscripted, and the move carries
  bytes — a record holding a byte that is not a character arrives intact.
- ✅ **FD `RECORD` clause — variable-length records.** All three spellings:
  `RECORD CONTAINS n CHARACTERS` (fixed), `RECORD CONTAINS n TO m CHARACTERS`
  (variable; the record description the `WRITE` names gives the length), and
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (the data item *is* the length — set before a `WRITE`, set back by a `READ`,
  and clamped to the declared range). An FD whose `01` records differ in size is
  variable-length whether or not it says so. A variable-length file stores each
  record's length with the record, so its bytes are **not** interchangeable with
  a fixed-length file's; a fixed-length file is unchanged.
- ✅ **An FD's `01` records describe one record area.** A `READ` delivers the
  bytes through every record description; a `WRITE` sends the whole area, so what
  another record description put where the written one has `FILLER` shows
  through.
- ✅ **`FILLER` occupies its bytes in an FD record**, and
  `SIGN IS SEPARATE CHARACTER` makes a signed DISPLAY item one character wider
  than its digit positions.
- ✅ **FD `LINAGE` takes data-names as well as integers** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. The page is
  measured from those items at each `WRITE`, so a program may resize it while it
  runs. `LINAGE-COUNTER` is one when the file is opened.
- ✅ **A sequential `READ` after `AT END` is `46`, not a second `10`.** The
  `AT END` left no valid next record, so reading on is a different error from
  reaching the end. `46` is a class‑4 status, so neither `AT END` nor
  `NOT AT END` runs for it — the file's `USE` declarative is what handles it.
  A fresh `OPEN`, or a successful `START`, establishes a record again.
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
- ✅ **`REWRITE` on a record-SEQUENTIAL file** replaces the record the last
  `READ` delivered, in place, and leaves the read position where it was — the
  next `READ` still gives the record that follows. The statuses it owes:
  **`49`** when the file is not open `I-O`, **`43`** when no successful `READ`
  established a record (including after `AT END`, and on a second `REWRITE` with
  no `READ` between), and **`44`** when the new record is not the same length as
  the one read — on a `DEPENDING ON` file the item's value is that length, which
  is how a program asks for a different one.
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
  An item whose PICTURE carries **no operational sign** is `NUMERIC` only when
  every character position holds a digit — `PIC X(5)` holding `"+1234"`,
  `"1.234"` or `"12 45"` is **not** numeric. *(Before 1.62.40 the test parsed
  the characters as a number, so a sign, a decimal point, an exponent and
  surrounding spaces were all accepted.)*
- ✅ **A user-defined `CLASS` operand may be an ordinal position** — `CLASS
  ORDINAL-A-ONLY IS 66` names the 66th character of the native set — and the
  operand may sit on its own source line. The same holds for `ALPHABET`.
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
- ✅ **`NOT` before an abbreviation *object* negates the relation**, not the
  object: `a > b OR NOT c` is `a > b OR NOT (a > c)`. The `NOT <relational
  operator>` spelling (`AND NOT < x`) is the operator form and is unchanged, and
  a `NOT` that opens an ordinary condition — `NOT (…)`, `NOT x = y`,
  `NOT x NUMERIC` — keeps its own meaning. *(Before 1.62.42 the object form was
  read as "the object is non‑zero", which gives the same answer only when the
  object happens to hold zero.)*
- ✅ **A condition‑name declared on a group tests the group's bytes.** A group
  owns no storage of its own — it *is* its children — so
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compares against the
  six characters the record holds.
- ✅ **A figurative constant is repeated to the size of the other operand**, and
  that includes one written as an 88's `VALUE`: `88 B VALUE QUOTE` on a
  `PIC X(4)` host is four quotes, and `88 D VALUE ALL "BAC"` is `"BACB"`.
  `ALL literal` is sized in **both** directions — `IF X EQUAL TO ALL "BA"` on a
  ten‑character `X` compares against `"BABABABABA"`, not `"BA"` padded with
  spaces.

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
  ✅ An unrecognised `FUNCTION` name is a **compile error** naming the function,
  with a suggestion when a real one is close enough to be a likely typo. It used
  to parse and return **0** at runtime, which turned a misspelling into a
  confidently wrong answer (1.62.15).
- ✅ Literals: integer, decimal, string, all figurative constants
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **A figurative constant fills its whole receiver**, including
  `HIGH-VALUE` — `MOVE HIGH-VALUE TO <PIC X(10)>` is ten `0xFF` bytes, and into
  a group it is distributed across the children. An alphanumeric-edited receiver
  still places its insertion characters, so `PIC XX0XXBXXX` holds
  `FF FF '0' FF FF ' ' FF FF FF`. Under a `PROGRAM COLLATING SEQUENCE` the
  constant names an ordinary character and that character fills instead.
  ⚠️ `HIGH-VALUE` is the **byte** `0xFF`, not a character. Reading a group
  operand, editing and every move path carry it byte for byte, but
  **reference modification is not yet byte-accurate** — `IF X (1:1) =
  HIGH-VALUE` is false for an item that genuinely holds `0xFF`.
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
- ✅ **Conformance flagging** (`cobolt_semantic::flagging`) — the standard asks a
  conforming implementation to be able to tell a program which of the features
  it uses sit outside a chosen conformance level. Two analyses answer that:
  - `flag_obsolete` — the COBOL‑85 **obsolete‑element** set: the five optional
    IDENTIFICATION DIVISION paragraphs, `MEMORY SIZE`, `ALTER`, `STOP` with a
    literal, and `GO TO` with no procedure‑name.
  - `flag_high_subset` — everything above the **high subset**, from `COMPUTE`,
    `EVALUATE` and `INITIALIZE` through `CORRESPONDING`, reference modification,
    qualification, `SET … TO TRUE` and a fourth subscript, down to continuing a
    *word* or a *numeric literal* across a card boundary. (Continuing an
    **alphanumeric** literal is in subset and is not reported.)

  Neither is error checking, and neither runs on an ordinary build: every
  construct they name is valid COBOL‑85 that RustCOBOL implements and executes.
  They are separate entry points precisely so a normal compile never starts
  warning about `AUTHOR` or about `COMPUTE`. NIST `NC302M`, `NC303M` and
  `NC401M` validate them — 7, 4 and 40 flags, all matched.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — the character that fills
  a currency position in an edited PICTURE. It **replaces** `$` rather than
  joining it, so once a program declares one, `$` is no longer a picture
  character there:
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` then reads `      <.00`, and `MOVE 1234` reads
  ` <1,234.00` — the floating run behaves exactly as `$$$,$$$.99` does. A
  **letter** currency symbol works the same way: `CURRENCY SIGN IS "W"` makes
  `PICTURE WWWWW` a five-position floating currency string, so `MOVE 12` reads
  `  W12`. *(Before 1.62.40 a run of a letter symbol was read as one word and
  rejected, so only `$` floated.)* The
  literal must be one character, and COBOL‑85 forbids one that would collide
  with a picture character or separator: not a digit, not one of
  `A B C D E G N P R S V X Z`, and none of `space * + - , . ; ( ) " / =`.
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
  , .`). The currency symbol is `$` unless `SPECIAL-NAMES. CURRENCY` named
  another — see **Expressions, literals, USAGE** above. **`P` is a decimal
  scaling position** — a digit position the item spans
  but does not store: `PIC S999PP` holds three digits standing for hundreds
  (`MOVE 12300` stores it exactly; `MOVE 12345` stores 12300), and `PIC PP99`
  holds two standing for ten‑thousandths. The positions the `P`s occupy always
  read back as zero and take **no bytes** in a record layout.
- ✅ **Check protection fills the whole item.** A zero value in a picture whose
  digit positions are all `*` fills every character position with asterisks —
  the fractional digits, the grouping commas, a fixed `$`, and a trailing `CR`
  or `DB` alike — leaving only the decimal point itself: `PIC $**.**CR` holding
  zero reads `***.****`, and `PIC *,***.**` reads `*****.**`. A **non**-zero
  value protects only the leading zeros, so the fixed `$` keeps its own position
  (`-2.34` → `$*2.34CR`). *(Before 1.62.37 `CR`/`DB` contributed one asterisk
  instead of the two character positions they occupy, so such an item came back
  one character short of its own width.)*
- ✅ **A numeric literal moves its characters, as written.** To an alphanumeric
  receiver a literal contributes the digits the program typed, left‑justified
  and space‑padded — `MOVE 2 TO <PIC X(4)>` is `"2   "`, and
  `MOVE 060820000200 TO <six PIC 99 children>` fills them
  `06 08 20 00 02 00`. The **receiver's** width never pads the literal; only its
  own written width does. *(Before 1.62.38 the lexer kept only the value, so a
  leading zero was lost and every following character shifted one place left.)*
- ✅ **A relation between a numeric and a nonnumeric operand is nonnumeric**
  (COBOL‑85 VI‑89 6.15.4 GR2). The numeric operand is treated as though moved to
  an alphanumeric item of **its own size**, which transfers its character
  positions and **not its operational sign**: `PIC S9(18)` holding
  `-123456789012345678` compares **equal** to `PIC X(18)` holding
  `"123456789012345678"`. Three conditions bound the rule — the numeric operand
  must be an **integer**; "nonnumeric" is decided by the **declaration**, so a
  `PIC 99` child holding characters after a group `MOVE` is still numeric — and
  a **group** is nonnumeric whatever its children are, so `PIC 9(5)` holding
  12345 against a ten‑byte group holding `"0000012345"` is `"12345     "` and
  unequal; and `ALL literal` takes the size of the other operand. *(Before 1.62.38 the
  comparison was algebraic whenever the text side happened to parse as a
  number.)*
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
- ✅ `VALUE` (numeric/signed/alphanumeric/figurative/`ALL`). **`VALUE ALL
  "literal"` repeats its unit across the whole item** — `PIC X(6) VALUE ALL
  "ABC"` is `"ABCABC"` and `PIC X(9) VALUE ALL "XY"` is `"XYXYXYXYX"`.
  *(Before 1.62.40 only the single-character figurative constants filled their
  item and `ALL "literal"` left it holding spaces.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — a **live** second reading of the same bytes. It adds no
  storage (so it does not widen the group that holds it), and a write through
  either description is visible through the other:
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` then reads back through `RESULT-A`.
  ⚠️ **Caveat:** an overlay larger than 256 expanded storage slots (a redefined
  10×10×10 table, say) keeps per‑description storage instead — refreshing it on
  every write would walk a thousand occurrences twice.
- ✅ **Overlays nest.** A `REDEFINES` inside a record that is itself redefined
  is reached in both directions, however deep: writing two bytes through a
  01‑level redefinition reaches the redefined record, the `REDEFINES` of a group
  inside it, and the `REDEFINES` of an item inside *that* — including an 88
  declared on the innermost one. Each description is re‑rendered once per write.
  *(Before 1.62.42 a key belonging to more than one overlay kept only the
  last‑declared one, and a single guard stopped the chain after its first hop.)*
- ✅ **An unnamed description is still a description.** `02 FILLER REDEFINES
  <item>.` redescribes its target's bytes under no name of its own, and a write
  to the target is visible through its children. Several children divide those
  bytes between them, in layout order — the overlay is *not* an alias of its
  first child. Two `FILLER REDEFINES` of one item are two independent readings,
  each starting at the target's **first** byte. *(Before 1.62.36 an unnamed
  redefining group was given no storage key at all, so its children read as
  spaces however the target had been filled.)*
- ✅ **A duplicated name inside an overlay** resolves to the same storage the
  rest of the program reaches: `TAB-A` declared under two different groups keeps
  one reading per declaration. *(Before 1.62.36 the overlay's initial copy was
  keyed from a path missing its outer qualifiers, which only a duplicated name
  can tell apart — so exactly the case that needs the qualifier lost it.)*
- ✅ `JUSTIFIED [RIGHT]` — **stores right‑aligned**, on an *alphanumeric* or an
  *alphabetic* item. A sender narrower than the receiver is padded on the left;
  a sender wider than it keeps its **right** end, losing its leftmost
  characters — the opposite of the ordinary rule. *(Before 1.62.40 the clause
  was recorded only for alphanumeric items, so `PICTURE A(5) JUSTIFIED RIGHT`
  parsed and then left‑aligned like any other item.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — accepted;
  `SIGN … SEPARATE` does not yet change how the item is stored.
- ✅ **A `REDEFINES` at the 01 level may describe more storage than the item it
  redefines**, and the bytes past that item's end belong to whichever
  description is long enough to name them. Writing through a shorter
  description leaves the longer one's tail alone.
- ✅ **A `REDEFINES` overlay carries the redefined item's bytes**, including
  into a numeric peer: a `PIC S9(18)` overlay of an `X(18)` holding
  `"00ABCDEFGHI  4321 "` reads those characters back, and `IS NUMERIC` answers
  **no** for them. When the bytes do spell digits the numeric reading is
  unchanged.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **real condition‑names**: the
  level‑88 binds to its host item; testing checks the host against the VALUEs /
  ranges, and `SET 88-name TO TRUE` stores a satisfying value into the host.
- ✅ **A condition‑name may be declared under more than one group, and `OF`/`IN`
  tells them apart** — exactly as it does for a data name, and intermediate
  levels may be skipped:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  The subscript belongs to the host item, so it selects which occurrence the
  VALUEs are tested against. An **unqualified** reference to a duplicated
  condition‑name is ambiguous in COBOL‑85; the runtime takes the first
  declaration, the same rule it applies to an ambiguous data name.
- ✅ `USAGE INDEX` declares an integer index register (`SET`/`SEARCH` use it);
  `USAGE POINTER` — see **Pointers** above.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — a regrouping alias;
  reading concatenates the covered items, writing distributes by field width.
  - ✅ **A 66 is qualified by the record it regroups**, exactly as a data item
    is qualified by the group above it, so the same 66 name may be declared once
    per record and told apart with `OF`/`IN`:
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. This works on reads and
    writes alike, and a 66 wins over an ordinary data item that happens to share
    its name. The operands of the `RENAMES` clause resolve in that same record,
    so a duplicated `NAME-2` names this record's.
  - ✅ **A covered table contributes every occurrence**, not just its first:
    `66 R RENAMES ITEM-1 THRU TABLE-2`, where `TABLE-2` holds
    `03 T PIC XXX OCCURS 5`, is 20 characters wide.
  - ✅ **A 66 over exactly one item *is* that item** — same PICTURE, same
    category, same storage. `66 R RENAMES W` where `W` is `PIC 9(4)` is a
    four-digit numeric item, so `ADD 3500 TO R` with 8000 in it raises
    `ON SIZE ERROR` and leaves it unchanged.
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
4. ✅ **Resolved (1.62.15).** An unrecognised intrinsic‑function name used to
   return **0** silently, so a program computed a confidently wrong answer from
   a typo. It is now a **compile error** that names the function and suggests
   the nearest real one when there is a close enough match
   (`cobolt-semantic/src/resolver.rs`, `Expr::FunctionCall`). Kept here because
   the silent‑zero shape is the trap items 5 and 6 still carry.
5. ⚠️ **An invalid `ACCESS MODE` / `ORGANIZATION` value is swallowed without a
   diagnostic** — the same trap again, and this one is triggered by an ordinary
   user typo. `ACCESS MODE IS` accepts only `SEQUENTIAL`, `RANDOM` or `DYNAMIC`
   (`INDEXED` is an *organization*, not an access mode), but the SELECT clause
   parser tests those three and lets anything else fall through to the generic
   "skip an unknown token" arm, so the file silently keeps the default
   `SEQUENTIAL` and misbehaves at run time instead of failing to compile.
   `ORGANIZATION IS` has the identical shape (`cobolt-parser/src/parser.rs`,
   the `Token::Access` arm and the organization arm above it). Both should
   raise a clear compile‑time error naming the offending word. **No NIST module
   will ever catch this** — the suite writes only valid clauses, so every
   module can finish at 100 % with the gap still open. It is a user‑typo trap,
   and it needs a test of its own rather than a module score.
6. ⚠️ **`ALPHABET … IS EBCDIC` is accepted but leaves native (ASCII) ordering
   in force.** The literal phrase (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` and `STANDARD‑2` are all implemented and drive
   `PROGRAM COLLATING SEQUENCE` for real; only the EBCDIC table is missing, and
   naming it silently gets ASCII order. Same trap family as 4–6.
7. **The Communication module and Report Writer** — see
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
