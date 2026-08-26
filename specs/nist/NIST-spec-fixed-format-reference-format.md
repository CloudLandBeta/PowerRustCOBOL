# NIST-spec — the fixed-format reference format (columns 73-80)

- **Status:** ✅ **IMPLEMENTED 2026-08-25** (with
  `NIST-spec-literal-continuation.md`, which is inseparable from it — see §9)
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** **459 of 459 programs (100 %)** — nothing in CCVS85 compiles
  without this.
- **Baseline:** `raw` pass = **0 / 459**; discarding columns 73-80 alone lifts it
  to 39 / 459.
- **Result:** **0 → 224 / 459 (48.8 %)** with the two together. See §9.

## 1. Overview

COBOL-85's fixed reference format divides a source line into five areas:

| Columns | Area | Meaning |
|--------:|------|---------|
| 1-6 | sequence number area | ignored by the compiler |
| 7 | indicator area | space, `*`, `/`, `-`, `D` |
| 8-11 | Area A | division / section / paragraph headers, level 01 and 77 |
| 12-72 | Area B | everything else |
| 73-80 | identification area | **ignored by the compiler** |

PowerRustCOBOL honours the sequence area and the indicator column, but
**deliberately does not stop at column 72** — `flatten_fixed` in
`crates/cobolt-lexer/src/source.rs:243` carries a comment recording the operator
ruling of 2026-08-05 that removed the limit:

> **The 72-column limit is not enforced** (operator, 2026-08-05). […] every
> generated form `.cbl` opens with a banner whose `*` sits in column 7, so the
> whole file was classified fixed, and any `EXEC RUST` block in a form had each
> line chopped mid-token […]. Embedded Rust has no column rules at all.

That ruling is sound for PowerRustCOBOL's own sources and **must not be
reverted**. But CCVS85 is genuine card-image COBOL: every one of its 348,271
lines carries a program stamp in columns 73-80.

```
000100 IDENTIFICATION DIVISION.                                         NC1014.2
000200 PROGRAM-ID.                                                      NC1014.2
000300     NC101A.                                                      NC1014.2
```

With the limit off, `NC1014.2` becomes real tokens on every line. The result is
not "some programs fail" — it is that the parser cannot find `PROGRAM-ID` or
`PROCEDURE DIVISION` in **any** of the 459 programs.

## 2. Goals / Non-goals

- **Goals:** compile genuine fixed-format COBOL-85 source correctly, including
  the identification area, without regressing PowerRustCOBOL's own sources.
- **Non-goals:**
  - Reinstating a global column-72 truncation. That is the 2026-08-05 defect.
  - Enforcing Area A / Area B *placement* rules (see §6).

## 3. User stories

- As a COBOL developer migrating a card-image codebase from Fujitsu PowerCOBOL
  or isCOBOL, I want my sequence-stamped source to compile unchanged, so that I
  do not have to strip columns 73-80 from every file first.
- As a PowerRustCOBOL form developer, I want my generated `.cbl` and its
  `EXEC RUST` blocks to keep running past column 72, so that nothing regresses.

## 4. Requirements (EARS)

- **R1 (optional):** Where a compilation unit is in **strict reference format**,
  the system shall ignore columns 73-80 of every source line.
- **R2 (ubiquitous):** The system shall provide an explicit way to select strict
  reference format per compilation unit. Selection shall not be inferred from
  the presence of a `*` in column 7 — that heuristic is what misclassified
  generated form sources in 2026-08-05.
- **R3 (state):** While strict reference format is *not* selected, the system
  shall behave exactly as today: sequence area and indicator column honoured,
  the line running as far as the developer typed.
- **R4 (ubiquitous):** The system shall keep `SourceFormat::Free` unchanged;
  free format has no column rules.
- **R5 (constraint):** The system shall not truncate at column 72 inside an
  `EXEC RUST … END-EXEC` block under any format.
- **R6 (constraint):** The system shall not delete or rewrite developer source.
  Truncation is a *read-time* rule applied to the token stream, never a
  transformation written back to the file (GOLDEN RULE — user code is sacred).

### Suggested selection mechanism

Three candidates; the plan phase picks one:

1. A new `SourceFormat::FixedStrict` alongside `Fixed` and `Free`, chosen by the
   caller. `rcrun` exposes it as `--format fixed-strict`, mirroring the existing
   `COBOLT_FIXED=1` escape hatch in `crates/cobolt-cli/src/main.rs:883`.
2. A per-project setting in `cobolt.toml`.
3. `>>SOURCE FORMAT IS FIXED` style directive in the source itself.

Option 1 is the smallest change and is what the CCVS85 harness needs; options 2
and 3 can follow later without invalidating it.

## 5. Acceptance criteria

- [x] AC1 — A fixed-format file whose columns 73-80 contain `NC1014.2` compiles
      identically to the same file with those columns blank, under strict
      reference format. → `strict_drops_the_identification_area`
- [x] AC2 — Under the default (non-strict) format, a line of 200 characters is
      still tokenized in full; the `egui`/`EXEC RUST` regression of 2026-08-05
      does not reappear. A regression test pins this. →
      `strict_does_not_change_relaxed_fixed` and
      `fixed_format_does_not_truncate_at_column_72`
- [~] AC3 — **Not reproducible as written, and the criterion was wrong.** It
      assumed the strict format would isolate the column rule, so that a strict
      run would land on the harness's `col72` figure of 39 / 459. It does not:
      `SourceFormat::FixedStrict` also joins continuation lines and reads a
      non-standard column-7 selector as ordinary source, so the same run scores
      **224 / 459**. The underlying claim — that the column rule alone accounts
      for the gap between 0 and 39 — is still evidenced by the harness's own
      `col72` pass, which does exactly that and is retained for the purpose.
      Nothing to fix; the criterion mis-stated what the implementation would be.
- [x] AC4 — Multi-byte characters in columns 60-80 do not panic and do not split
      a character; the existing `char_boundary_at_col` discipline is preserved.
      → `strict_clips_multibyte_characters_on_a_boundary`
- [x] AC5 — A line shorter than 73 characters behaves exactly as before.
      → `strict_leaves_a_short_line_alone`

## 6. Deliberately not in scope: Area A / Area B enforcement

CCVS85 program **NC113M** exists to verify "correct use of Area A". It places
division headers, level numbers and paragraph names at columns 9, 10, 11 and 15,
and splits headers across lines:

```
000100    IDENTIFICATION DIVISION.
003600    DATA
003700     DIVISION.
011900    PROCEDURE
012000      DIVISION
```

**Measured: PowerRustCOBOL already parses all of these correctly.** A probe
reproducing every Area A variant in NC113M parses clean. NC113M nonetheless
fails today — for an unrelated reason, literal continuation, covered by
`NIST-spec-literal-continuation.md`.

So this spec **adds no Area A placement checks**. Rejecting Area B paragraph
names would be new strictness with no NIST program demanding it, and would risk
existing user code. Nothing already implemented is removed.

## 7. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** critical — the generated form `.cbl` path must
  keep the current non-truncating behaviour. AC2 pins it.
- **Docs:** `docs/developers-guide-en.md` gains a short "source formats" note
  aimed at a PowerCOBOL/isCOBOL developer explaining when to pick strict
  reference format; `docs/cobol85-supported-syntax-en.md` records the rule.
- **Fix vs feature:** **fix** — conformance with the COBOL-85 reference format.

## 8. Open questions

- Q1: ~~Which selection mechanism (§4)?~~ **Resolved: option 1.**
- Q2: Should strict reference format also *warn* when text is found in columns
  73-80 of a file compiled in the default format, to help a developer notice
  they wanted strict mode? A warning is safe; an error is not. **Still open.**

## 9. What shipped — 2026-08-25

Implemented together with `NIST-spec-literal-continuation.md`. The two could not
sensibly be split: a continued alphanumeric literal runs to column 72, so
reassembling it requires the column rule, and applying the column rule without
reassembling literals leaves 396 programs broken.

**`SourceFormat::FixedStrict`** — a third variant beside `Fixed` and `Free`
(`crates/cobolt-lexer/src/source.rs`). `SourceFormat::Fixed` is untouched, so
the 2026-08-05 relaxed reading that generated form sources and `EXEC RUST`
blocks depend on still behaves exactly as before.

`flatten_fixed_strict()` applies: sequence area ignored; column 7 indicator
(`*` `/` comment, `D` debugging line treated as a comment, `-` continuation);
source in columns 8-72; columns 73-80 discarded; continuation lines joined for
both literals and words, with the continued fragment padded to column 72 when it
leaves a literal open. One output line is emitted per input line, so spans keep
pointing at the physical line the developer wrote. Wired into `Lexer::new`,
`preprocess` and the COPY preprocessor's `flatten`.

**CLI:** `rcrun run|check --source-format=<free|fixed|fixed-relaxed|auto>`, and
`COBOLT_SOURCE_FORMAT` as its default. `fixed` selects the classic reference
format; `fixed-relaxed` selects the existing lenient reading; `auto` (the
default) keeps the historical behaviour, including `COBOLT_FIXED=1`.

**One judgement call, recorded deliberately.** A column-7 character that is not
a COBOL-85 indicator is read as **ordinary source** rather than rejected.
CCVS85 uses the indicator area as a *selector*, marking optional lines with `Y`,
`P`, `C`, `S` and eleven other letters (4,830 `Y` lines alone). Rejecting them
would fail the suite; dropping them would silently delete code, which GOLDEN
RULE "user code is sacred" forbids. This makes
`NIST-spec-harness-and-baseline.md` R3 — the harness's own selector
normalisation — unnecessary, which is why the measured result beats the 199 that
harness preparation reached.

**Measured, on the untouched 28 MB distribution with no preparation at all:**

| Pass | Clean / 459 |
|---|---:|
| `raw` — relaxed `Fixed` | 0 (0.0 %) |
| `strict` — `SourceFormat::FixedStrict` | **224 (48.8 %)** |

Per module: NC 25/95, SQ 47/85, IC 32/47, IF 21/45, IX 31/42, ST 27/40,
RL 30/35, SM 4/17, DB 5/15, SG 0/13, CM 0/9, RW 0/6, OBIC 2/3, others 0.
In scope (excluding the 25 programs of
`NIST-spec-out-of-scope-modules.md`): **222 / 434 (51.2 %)**.

**Tests:** nine cases in `crates/cobolt-lexer/src/source.rs`, covering the
identification area, literal continuation (NC113M's 54-hyphen `HYPHEN-LINE`),
short-line padding to column 72, word continuation, one-output-line-per-input,
comment and `D` indicators, the doubled-delimiter escape, selector indicators,
and a guard that relaxed `Fixed` still runs past column 72.
