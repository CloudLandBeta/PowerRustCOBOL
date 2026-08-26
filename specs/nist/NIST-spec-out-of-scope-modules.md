# NIST-spec — modules held out of scope, and what "success" means without them

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Scope:** 25 of the 459 programs (5.4 %).

## 1. Overview

The operator's instruction was to spec the fixes needed to run CCVS85
successfully, "except what is not supported by RustCOBOL such as COMMUNICATION
SECTION, OO etc." This spec records **exactly** which programs that excludes, so
the target is a number rather than a hand-wave, and so nobody later reads a
score of 433 as a failure to reach 459.

**This spec removes nothing that is already implemented.** It only declares what
is not being added.

## 2. Held out of scope

### 2a. Communication module — CM, 9 programs

`CM101M CM102M CM103M CM104M CM105M CM201M CM202M CM303M CM401M`

The `COMMUNICATION SECTION`, `CD` entries and the `SEND`, `RECEIVE`, `ACCEPT …
MESSAGE COUNT`, `ENABLE` and `DISABLE` verbs. Measured first errors:

```
CM303M | 002800     DISABLE INPUT COMMNAME WITH KEY CNAME1.
CM401M | 003700     DISABLE INPUT COMMNAME WITH KEY CNAME1.
```

This module targets 1980s teleprocessing monitors — message queues owned by a
transaction manager. There is no such runtime here and no user asking for one.
It was removed from COBOL in later standards.

**Ruling: out of scope. Not implemented, not stubbed.**

### 2b. Report Writer — RW, 6 programs

`RW301M RW302M` and 4 more.

`REPORT SECTION`, `RD` entries, `INITIATE`, `GENERATE`, `TERMINATE`, control
breaks and sum counters. Measured first errors:

```
RW301M | 007300     INITIATE RFIL2.
RW302M | 007100     INITIATE RFIL2.
```

Report Writer is a large declarative sub-language — arguably the largest single
feature in COBOL-85 not yet present. PowerRustCOBOL's answer to reporting is the
form designer and `pdf_export.rs`.

**Ruling: out of scope for the NIST effort.** Unlike CM, this one is defensible
as a future **feature** (it is a capability beyond what the IDE offers today,
so it would be announced on forum f=96, not f=97). It is not part of this
fix programme.

### 2c. Obsolete-feature variants — OBSQ / OBIC / OBNC, 9 programs

`OBSQ` (4), `OBIC` (3), `OBNC` (2). These re-test earlier modules using elements
COBOL-85 marks obsolete, and expect the compiler to **flag** them.

Their language content is largely covered by the other specs — OBNC1M's first
error is an `INSTALLATION` comment-entry
(`NIST-spec-identification-division-comment-entries.md`), OBIC1A's is a section
priority number (`NIST-spec-segmentation.md`). What is *not* covered is the
flagging itself.

**Ruling: the language content is in scope via the other specs; obsolete-feature
flagging is out of scope.** These programs may compile and run but will not
produce the flagging messages CCVS85 looks for, so they are excluded from the
target score. If the operator later wants flagging, it is its own spec.

### 2d. Object-Oriented COBOL — 0 programs

Named in the instruction, but CCVS85 predates OO COBOL entirely. There are no OO
programs in the suite. Recorded here only to close the question.

## 3. The resulting target

| | Programs |
|---|---:|
| Total COBOL programs in CCVS85 | 459 |
| less Communication (CM) | −9 |
| less Report Writer (RW) | −6 |
| less obsolete-flagging variants (OBSQ/OBIC/OBNC) | −9 |
| less the EXEC85 driver (see §4) | −1 |
| **In-scope target** | **434** |

Baseline when this spec was written: **199 / 459 overall**, and since two of the
excluded programs were already clean (OBIC scores 2 / 3), **197 / 434 (45.4 %)**
in scope.

**Current, after the source-format fix (2026-08-25):** **224 / 459** overall →
**222 / 434 (51.2 %)** in scope, **212** in-scope failures, **25** N/A.

## 4. A note on EXEC85

`EXEC85` is not a test. It is the CCVS *executive* — the COBOL program NIST
ships to split the population file and drive the suite. Our harness
(`NIST-spec-harness-and-baseline.md`) replaces it in Rust, so `EXEC85` does not
need to compile for the suite to run.

It is excluded from the target for that reason, not because of a language gap.
Its measured first error is an `INSTALLATION` comment-entry, which
`NIST-spec-identification-division-comment-entries.md` fixes anyway — so it may
well compile as a side effect. Good, but not required.

## 5. Requirements (EARS)

- **R1 (ubiquitous):** The harness shall tag every program with its module and
  shall report the in-scope score and the full score separately.
- **R2 (event):** When a program uses a construct from an out-of-scope module,
  the system shall emit a clear diagnostic naming the construct and stating it
  is not supported — not a generic parse error, and never silence.
- **R3 (constraint):** The system shall not stub out-of-scope verbs as no-ops.
  A `GENERATE` that does nothing is worse than one that refuses, because the
  program then reports a wrong answer instead of an honest failure. This is the
  same lesson as `NIST-spec-relative-organization.md` R6.
- **R4 (constraint):** Nothing already implemented shall be removed to satisfy
  this spec.

## 6. Acceptance criteria

- [x] AC1 — ✅ **done 2026-08-25.** The harness prints a PASS / FAIL / N-A block:

      --- PASS / FAIL / N-A ---
        PASS  222 / 434   (51.2% of the in-scope suite)
        FAIL  212 / 434   (48.8%)
        N-A    25 / 459   (out of RustCOBOL scope: CM, RW, OB*, EXEC85)

      and marks each out-of-scope module `N-A` in the per-module table.
      (The 197 / 434 figure quoted in §3 was the pre-source-format baseline;
      222 / 434 is current.)
- [ ] AC2 — A program containing `COMMUNICATION SECTION` is rejected with a
      diagnostic naming it, not with `unexpected token`.
- [ ] AC3 — Same for `REPORT SECTION`, `INITIATE`, `GENERATE`, `TERMINATE`,
      `SEND`, `RECEIVE`, `ENABLE`, `DISABLE`.
- [ ] AC4 — `docs/cobol85-supported-syntax-en.md` lists these modules in its
      avoid-list with this spec's reasoning.

## 7. Constraints & steering check

- **i18n:** R2's diagnostics are compiler output, not IDE UI strings, so no `Tr`
  keys are needed. Confirm during `/plan` whether any surface in the IDE.
- **Docs:** the avoid-list in `docs/cobol85-supported-syntax-en.md` currently
  claims "The COBOL-85 verb / clause set is **fully covered**". That sentence
  must go, and this spec's exclusions must replace it.
- **Fix vs feature:** the diagnostics in R2 are a **fix**. Report Writer, if the
  operator ever wants it, is a **feature** (f=96).

## 8. Open questions

- Q1: Is Report Writer genuinely unwanted, or wanted later? It is the one
  exclusion here with real user value. **Operator ruling wanted.**
- Q2: Should the DB module (15 programs) join this list? It is obsolete in later
  standards, but it is cheap and already specified
  (`NIST-spec-debugging-module.md` Q2). Recommendation: keep it in scope.
- Q3: Confirm the −9 for OBSQ/OBIC/OBNC. If flagging is out of scope but the
  programs otherwise run and self-report `PASS` on everything except the
  flagging checks, they might be partially creditable. Decide how the harness
  scores a partial pass.
