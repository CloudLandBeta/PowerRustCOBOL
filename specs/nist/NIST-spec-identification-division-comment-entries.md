# NIST-spec — IDENTIFICATION DIVISION comment-entry paragraphs

- **Status:** ✅ **IMPLEMENTED 2026-08-25** (version 1.62.11)
- **Result:** NIST in-scope PASS **237 → 241 / 434** (54.6 % → 55.5 %); Debug 5 → 9.
  The 32-program `expected Division, found …` bucket is **eliminated**. The gain
  is smaller than that bucket because 9 of the 32 are Communication programs
  (N/A) and most of the rest hit a second blocker immediately after — see §10.
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** 34 programs use `INSTALLATION`, 34 use `SECURITY`, 3 use
  `DATE-COMPILED`. **`REMARKS`: zero** — re-measured 2026-08-25; the original
  count of 1 matched `VALUE "REMARKS"` inside a string literal, not a paragraph
  header. At the 237 / 434 baseline this is the **largest remaining bucket**:
  32 programs fail on the comment-entry text and 6 more on `DATE-COMPILED`.
- **Plan:** [`NIST-plan-identification-division-comment-entries.md`](NIST-plan-identification-division-comment-entries.md)

## 1. Overview

COBOL-85's IDENTIFICATION DIVISION has six optional paragraphs whose content is
a **comment-entry**: `AUTHOR`, `INSTALLATION`, `DATE-WRITTEN`, `DATE-COMPILED`,
`SECURITY` (and the obsolete `REMARKS`). A comment-entry is free text. It may
contain any character, including reserved words and periods, and it is
terminated by **the next entry beginning in Area A** — not by a period.

PowerRustCOBOL gets three things wrong here, and each has its own failure
signature in the baseline.

### 1a. `DATE-COMPILED` is a keyword the parser never handles

`DATE-COMPILED` lexes to `Token::DateCompiled`
(`crates/cobolt-lexer/src/keywords.rs:58`). The IDENTIFICATION DIVISION loop in
`crates/cobolt-parser/src/identification.rs:73` matches only `Token::Author`,
`Token::DateWritten` and a generic `Token::Identifier(_)`; `DateCompiled` falls
through to `_ => break`. The division ends early and the parser then demands a
division header where a paragraph name sits:

```
000600 DATE-COMPILED.  22ND AUG 1988.        →  L6 expected PROCEDURE DIVISION
```

Measured on **NC303M** (a 32-line program that exists to check obsolete-feature
flagging) and reproduced in a minimal probe.

### 1b. The comment-entry terminates on a period, not on Area A

`collect_comment_text` (`identification.rs:110`) consumes tokens up to the next
period. CCVS85's `EXEC85` writes a two-line comment-entry, each line ending in a
period:

```
000600 INSTALLATION.
000700     "ON-SITE VALIDATION, NATIONAL INSTITUTE OF STD & TECH.     ".
000800     "COBOL 85 VERSION 4.2, Apr  1993 SSVG                      ".
000900 ENVIRONMENT DIVISION.
```

The first period ends the entry; line 000800 is then read as a statement.

### 1c. Reserved words inside a comment-entry are treated as real

`collect_comment_text` also stops at `Token::Environment`, `Token::Data` and
`Token::Procedure`. CCVS85 puts the word `DATA` inside a `SECURITY` entry:

```
000800     AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
```

Collection stops at `DATA`, the parser concludes the DATA DIVISION has started,
looks for `DIVISION`, finds `AND`, and reports `expected Division, found …` —
the 33-program bucket. **CM101M** and **OBNC1M** are the examples.

## 2. Goals / Non-goals

- **Goals:** parse all six comment-entry paragraphs per COBOL-85, preserving
  their text where the AST already has a field for it.
- **Non-goals:**
  - Acting on the content. `DATE-COMPILED` is famously replaced by the compile
    date in some implementations; COBOL-85 permits but does not require it, and
    NIST does not test for it.
  - Adding obsolete-feature *flagging*. NC303M's comment says
    "Message expected for above statement: OBSOLETE", but flagging is a separate
    concern — see §6.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall recognise `AUTHOR`, `INSTALLATION`,
  `DATE-WRITTEN`, `DATE-COMPILED`, `SECURITY` and `REMARKS` as IDENTIFICATION
  DIVISION paragraph headers.
- **R2 (ubiquitous):** The system shall treat a comment-entry as free text: no
  token inside it shall be interpreted as a reserved word, a statement or a
  division header.
- **R3 (state):** While in strict reference format, the system shall terminate a
  comment-entry at the next line whose first non-blank character lies in Area A
  (columns 8-11), or at end of file.
- **R4 (state):** While *not* in strict reference format, the system shall
  terminate a comment-entry at the next line beginning with one of the
  IDENTIFICATION paragraph keywords or a division header keyword. This is a
  heuristic and shall be documented as such.
- **R5 (ubiquitous):** The system shall accept these paragraphs in any order and
  shall accept any subset of them, including none.
- **R6 (constraint):** The system shall not fail a compilation because a
  comment-entry is absent, empty, or contains a period.
- **R7 (ubiquitous):** The system shall populate the existing
  `IdentificationDivision` fields `installation`, `date_compiled` and `security`
  — currently hard-coded to `None` at `identification.rs:100` — rather than
  discarding the text.

## 4. Acceptance criteria

- [~] AC1 — NC303M parses clean. **Partially met — the criterion as written is
      not achievable by this spec.** The `DATE-COMPILED` cause *is* fixed:
      NC303M previously failed at line 6 on the paragraph itself, and now
      reaches **line 19**, where it stops on
      `ALTER NC303M-GOTO TO PROCEED TO NC303M-GOTO-2,` — a separator comma,
      owned by [`NIST-spec-separators.md`](NIST-spec-separators.md). Same for
      NC401M. Verified by direct measurement, not inferred.
- [x] AC2 — EXEC85's two-line `INSTALLATION` entry is captured, both lines
      (`comment_entry_of_quoted_lines`). The *program* still does not parse
      clean — it now reaches **line 1034** and stops on an unrelated `ELSE`.
- [x] AC3 — A `SECURITY` entry containing the words `DATA`, `PROCEDURE`,
      `ENVIRONMENT` and `DIVISION` parses clean and does not start a division.
- [x] AC4 — `IdentificationDivision.installation`, `.date_compiled` and
      `.security` are `Some(...)` when the paragraphs are present.
- [x] AC5 — The **32-program** `expected Division, found …` bucket loses its
      comment-entry members (CM101M-CM105M, OBNC1M and peers), and the
      **6-program** `expected PROCEDURE DIVISION` bucket loses NC303M and
      NC401M (`DATE-COMPILED`) and EXEC85 (two-line `INSTALLATION`).
      *(Written when the buckets were 33 and 48 and NC205A was still in the
      first; literal continuation has since shipped and removed it. Re-measure
      rather than trusting these figures.)*
      ⚠️ **The in-scope gain is smaller than 38.** Nine of the 32 are CM
      programs, which are N/A — they will start parsing and stay N/A.
- [x] AC6 — A program with no optional paragraphs at all still parses (no
      regression).

## 5. Note — this bucket has two owners

The 48-program `expected PROCEDURE DIVISION` bucket is **not** all this spec.
Measured breakdown:

- programs whose ID division contains `DATE-COMPILED` → **this spec** (3);
- programs whose failure is a swallowed continued literal → **
  `NIST-spec-literal-continuation.md`** (the large majority — NC108M, NC113M,
  SQ102A, IX101A and peers).

Both produce the identical end-of-stream diagnostic, which is why the bucket
looked homogeneous. Fix order does not matter, but the harness census must be
re-read after each so credit is assigned correctly.

## 6. Deliberately not in scope: obsolete-feature flagging

Several CCVS85 programs (NC303M, the `OB*` module) expect the compiler to
*flag* obsolete COBOL-85 elements — `DATE-COMPILED`, `ALTER`, `SEGMENT-LIMIT`,
segment numbers, `REMARKS`. Flagging is an optional conformance level, not a
requirement for compiling and running the suite, and adding warnings for
constructs PowerRustCOBOL already supports would be new noise for existing
users. It is recorded here and left out; if the operator wants it later it is
its own spec. **Nothing already implemented is removed.**

## 7. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator emits `AUTHOR` in generated
  `.cbl` banners; R5/R6 keep that working. A regression test should compile one
  generated form source.
- **Docs:** `docs/cobol85-supported-syntax-en.md` gains the six paragraphs.
- **Fix vs feature:** **fix**.

## 8. Open questions

- Q1: R4's free-format heuristic — should a comment-entry even be *allowed* in
  free format? COBOL-85 defines Area A only for fixed format. Recommendation:
  allow it with the keyword heuristic, since PowerRustCOBOL treats its own
  sources as free format and generated banners use `AUTHOR`.
- Q2: Should `date_compiled` be replaced with the actual compilation date, as
  some vendors do? Recommendation: no — store the source text verbatim
  (user code is sacred), and revisit only if a NIST program checks it.

## 10. What actually happened — measured 2026-08-25

**In-scope PASS: 237 → 241 / 434.** The plan estimated 8-15; the real gain is
**+4**, all of it in the Debug module (5 → 9). Recording why, because the gap
between estimate and result is the useful part.

The 32-program `expected Division, found …` bucket **is gone**. But a bucket
disappearing is not the same as programs passing:

| Where the 32 went | Programs |
|---|---:|
| Communication (CM) — now parse past the ID division, stop on `ENABLE` / `DISABLE`; **N/A, so no in-scope gain** | 9 |
| Report Writer (RW) — same shape, stop on `INITIATE`; **N/A** | 6 |
| Reached a second in-scope blocker immediately (separator comma, `SET`, `COPY`) | most of the rest |
| Actually passed | 4 (DB) |

Two things this run establishes, both worth carrying forward:

**1. Bucket size is an upper bound on the gain, never a prediction.** A
root-cause census counts *first* errors. Clearing one cause moves a program to
its next cause; it only reaches PASS if there is no next cause. Estimate from
the module mix (how many are N/A) and expect most in-scope programs to have a
second blocker.

**2. This fix exposed a lexer defect that had been masked.** Six programs —
NC215A, SG104A, SG105A, SG106A and peers — now fail with the end-of-file
`expected PROCEDURE DIVISION` signature. The cause is a **stray quotation mark
inside comment-entry prose**:

```
004100     THIS PROGRAM CHECKS THE COMPILER"S ABILITY TO HANDLE EIGHT
```

`COMPILER"S` opens a string literal, and the lexer's string rule crosses
newlines, so it runs to the next quotation mark anywhere in the file and takes
the rest of the program with it. COBOL-85 says a comment-entry is raw text, so
this is our defect, not the suite's.

It is **not fixable in this spec**: lexing happens before parsing, so the lexer
cannot know it is inside a comment-entry. The right fix is the one already
recorded as **R6 of [`NIST-spec-literal-continuation.md`](NIST-spec-literal-continuation.md)** —
a literal must not cross a line boundary, and an unterminated one must be
reported on its own line instead of swallowing the file. That was left open when
literal continuation shipped; it is now blocking 6 programs and should be
scheduled.
