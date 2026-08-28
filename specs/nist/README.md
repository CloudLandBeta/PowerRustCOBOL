# NIST CCVS85 conformance specs

The litmus test for RustCOBOL is `NIST/newcob.val,cbl` — the official NIST
COBOL-85 validation suite (CCVS85 4.0, 01 OCT 1992), 28 MB, 348,271 lines,
**459 COBOL programs** plus 51 COPY library members.

These specs are the plan for making it run, one fix at a time, through the
project's `/plan → /tasks → /analyze → /implement` pipeline. **Six have
shipped** (specs 1-5 and 15), and spec 7 is **partly** shipped — four of its
seven independent gaps.

## Where we stand

Front end only (lexer → parser → semantic analyser), measured with
`crates/cobolt-semantic/examples/nist_conformance.rs` on the untouched 28 MB
distribution:

| | Programs | Share | |
|---|---:|---:|---|
| ✅ **PASS** | **417** | **96.1 %** | of the 434 in-scope programs |
| ❌ **FAIL** | **17** | 3.9 % | of the 434 in-scope programs |
| ⬜ **N/A** | **25** | — | CM (9), RW (6), OBSQ/OBIC/OBNC (9), EXEC85 (1) |
| | **459** | | total in the suite |

**Compiling is the weaker claim.** The suite scores itself, so there is a second
number — programs that run to completion and report **zero failures**. Under
GOLDEN RULE #9 the work is one module at a time, and both numbers are reported
separately:

| Module | Compile | Execution (0 failures) | Measured |
|---|---:|---:|---|
| **NC (Nucleus)** | **92 / 95** | **28 / 95** | 1.62.21 |

```bash
cargo build --release -p cobolt-cli
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

Progress: **0 → 222** (source format, 1.62.8) **→ 237** (numeric literals,
1.62.10) **→ 241** (IDENTIFICATION comment entries, 1.62.11) **→ 242**
(literals confined to one line, 1.62.12) **→ 292** (separators, 1.62.13)
**→ 317** (intrinsics and statement grammar, 1.62.14) **→ 332** (unknown FUNCTION, digit-leading words, 1.62.15) **→ 376** (the `AT` in `[AT] END` is
optional; COPY library, 1.62.16) **→ 380** (`LINAGE`, 1.62.17) **→ 391**
(continuation-line operands, 1.62.18) **→ 396** (numeric-edited items are
numeric, 1.62.19).

Measured one change at a time, which is the only way the attribution below is
trustworthy:

| Change | PASS |
|---|---:|
| 1.62.12 baseline | 242 |
| a doubled delimiter inside a literal is one character | 244 |
| separator comma / semicolon; space-separated subscripts | 285 |
| a subscript after a complete qualified name | 292 |
| `ALL` as a subscript — a whole table as an intrinsic argument | 303 |
| `CLOSE … WITH LOCK`; signed `WHEN` literal; `PERFORM … TIMES` identifier | 314 |
| an integer count written on a continuation line | 317 |
| unknown `FUNCTION` → compile error; digit-leading user-defined words | 332 |
| the `AT` in `AT END` is optional | 365 |
| COPY library wired in; a literal confined to its line in the preprocessor | 372 |
| a leading decimal point in an arithmetic operand list | 376 |
| `LINAGE` and `AT END-OF-PAGE` (1.62.17) | 380 |
| continuation-line operands; optional `IS`; all-digit procedure names (1.62.18) | 391 |
| a numeric-edited item is a numeric item (1.62.19) | **396** |

**The Intrinsic Functions module is complete at 45 / 45**, and so is Indexed
I/O at 42 / 42.

Per module at 1.62.19: NC 74 / 95 · SQ 82 / 85 · IF 45 / 45 · IC 44 / 47 ·
IX 42 / 42 · ST 38 / 40 · RL 34 / 35 · SM 14 / 17 · SG 12 / 13 · DB 11 / 15.

The suite scored zero not because of 459 language gaps but because two
mechanical source-format rules were unimplemented — the 72-column limit and
continuation lines. Those two alone were worth 222 programs. The remaining
failures are genuine language gaps, and the root-cause census in
[spec 0](NIST-spec-harness-and-baseline.md#6a-current-baseline--2026-08-25-after-the-source-format-fix)
ranks them.

> **A bucket's size is an upper bound on the gain, never a prediction.** The
> census counts each program's *first* error. Clearing one cause moves a program
> to its next cause; it reaches PASS only if there is no next one. Spec 3
> cleared a 32-program bucket and gained 4 — 9 of the 32 were out-of-scope
> Communication programs and most of the rest had a second blocker. Estimate
> from the module mix, and re-measure rather than projecting.

## Reading order

Start with the baseline. Everything else sizes itself against it.

| # | Spec | Blast radius |
|---|------|--------------|
| 0 | [harness and baseline](NIST-spec-harness-and-baseline.md) | the instrument, the measured numbers, the construct census |
| 1 | [fixed-format reference format](NIST-spec-fixed-format-reference-format.md) | ✅ **shipped** — columns 73-80 |
| 2 | [literal continuation](NIST-spec-literal-continuation.md) | ✅ **shipped, R6 closed 1.62.12** — was 396 programs |
| 3 | [IDENTIFICATION comment entries](NIST-spec-identification-division-comment-entries.md) | ✅ **shipped** — was 38 first-errors |
| 4 | [numeric literals](NIST-spec-numeric-literals.md) | ✅ **shipped** — was 36 first-errors |
| 5 | [separators](NIST-spec-separators.md) | ✅ **shipped 1.62.13** — was the largest; worth **+48** |
| 6 | [user-defined words](NIST-spec-user-defined-words.md) | ✅ **shipped 1.62.15** — digit-leading words; NC 58 → 61, SG 0 → 10 |
| 7 | [statement grammar gaps](NIST-spec-statement-grammar-gaps.md) | ⚠️ **4 of 7 gaps shipped 1.62.14** — MULTIPLY receivers, EVALUATE class subject, INSPECT CONVERTING remain |
| 8 | [COPY and REPLACE](NIST-spec-copy-and-replace.md) | SM module, 4 / 17 |
| 9 | [SPECIAL-NAMES](NIST-spec-special-names.md) | parsed by skipping today |
| 10 | [RELATIVE organization](NIST-spec-relative-organization.md) | RL module, 35 programs |
| 11 | [nested programs](NIST-spec-nested-programs.md) | IC module, 32 / 47 |
| 12 | [debugging module](NIST-spec-debugging-module.md) | ⚠️ **the `D`-indicator prerequisite shipped 1.62.15**; the module itself (DEBUG-ITEM, declaratives) remains |
| 13 | [segmentation](NIST-spec-segmentation.md) | ⚠️ SG **10 / 13** after 1.62.15 — most of it came free with digit-leading words |
| 14 | [LINAGE and I-O-CONTROL](NIST-spec-linage-and-io-control.md) | SQ module |
| 15 | [intrinsic function gaps](NIST-spec-intrinsic-function-gaps.md) | ✅ **complete — R1-R6 shipped (1.62.14, R5 in 1.62.15). IF module 45 / 45.** |
| 16 | [out-of-scope modules](NIST-spec-out-of-scope-modules.md) | ✅ **settled 2026-08-26** — Report Writer declined as obsolete. CM, RW, OB\* stay out; target is 434, not 459 |

## Dependencies

```
1 fixed-format ──┬─→ 2 literal continuation ──→ 8 COPY/REPLACE     ✅ 1-4 done
                 └─→ (everything else)
5 separators      ──→ 11 nested programs        (PROCEDURE DIVISION USING lists)
6 user-defined words ─→ 13 segmentation         (all-numeric section names)
```

✅ **Closed at 1.62.12: spec 2's R6.** A quotation mark in ordinary prose —
`THE COMPILER"S ABILITY` inside a comment-entry — used to open a literal that
ran to the next quote anywhere in the file. Spec 3 surfaced it; a literal is now
confined to its line. Of the 6 programs it blocked, one passes and four advanced
to segment priority numbers, so **spec 13 is now the Segmentation module's real
blocker** rather than a symptom of this one.

Specs 1 and 2 came first, and had to: a swallowed program reported one
misleading end-of-stream error and hid every real diagnostic behind it. Each fix
since has reordered the census and surfaced something that was invisible before
— the separator-semicolon bucket after spec 1, the `SET` bucket after spec 4,
the stray-quote defect after spec 3, and segment numbers after R6. **Re-read the
census after each spec lands rather than trusting the previous ranking.**

## Three rules that shaped these specs

**Parsing clean is not passing.** A CCVS85 program prints its own `PASS`/`FAIL`
tally; that is the score. Two measured cases prove why: 30 of 35 RELATIVE
programs parse today and would then run with no engine behind them, and a
continued literal whose stray quotes happen to balance parses fine while holding
the wrong data.

**Silence is the worst failure.** Where a construct is out of scope, the
compiler must say so. Stubbing a verb as a no-op makes a program report a wrong
answer instead of an honest failure.

**Nothing implemented is removed.** Every spec is additive. Where conformance
would change existing behaviour, the spec flags it as an open question for the
operator rather than deciding unilaterally.

### ✅ All three were ruled on, 2026-08-26

| Question | Ruling |
|---|---|
| digit-leading words vs unspaced subtraction (spec 6 Q1) | **`B-C` is a data-name.** COBOL allows the hyphen in a name; an operator needs spaces around it. |
| `D` lines as comments without `WITH DEBUGGING MODE` (spec 12 Q1) | **Follow the standard — in fixed format only.** Free format has no indicator area, so a `D` there is an ordinary word. |
| unknown intrinsic name (spec 15 Q1) | **A compile error**, naming the nearest implemented function. |
| Report Writer (spec 16 Q1) | **No.** An obsolete language element; it stays out of scope. |

All four shipped in 1.62.15 except Report Writer, which is now closed rather
than pending.

## Classification

All of this is **fix** work, not feature work. CLAUDE.md rule #4 is explicit:
implementing a COBOL-85 construct that should already work, or making
non-conformant behaviour conform, is technical debt — announced on forum f=97.
The one exception is Report Writer, which would be a genuine new capability
(f=96) if the operator ever wants it; it is currently out of scope.

## The instrument

```bash
# the real path: untouched source, classic reference format
cargo run -p cobolt-semantic --example nist_conformance -- strict

# other passes: raw (before), col72, nist, nistdel (pre-fix reproductions)
# drill into one program
cargo run -p cobolt-semantic --example nist_conformance -- dump NC101A
# find where a silent failure begins
cargo run -p cobolt-semantic --example nist_conformance -- bisect NC113M
# write a prepared program to stdout
cargo run -p cobolt-semantic --example nist_conformance -- extract NC303M
# parse one hand-written fixed-format file
cargo run -p cobolt-semantic --example nist_conformance -- probe /path/to/probe.cbl
# construct census across the suite
cargo run -p cobolt-semantic --example nist_conformance -- features
```

The example is read-only: it reads `NIST/newcob.val,cbl` and changes no product
behaviour. It exists so every acceptance criterion in these specs is measurable
rather than asserted.

## 🔴 Execution scoring — added 1.62.15, and it changes the picture

`nist_conformance run` executes each program that compiles and reads the
program's **own** `PASS`/`FAIL` report, which is what CCVS85 exists to produce.

**Of 434 in-scope programs, 0 run to completion.**

| | Programs |
|---|---:|
| did not compile | 102 |
| **ran to completion reporting 0 failures** | **0** |
| looped until killed for output (>2 MB) | 170 |
| timed out (>20 s) | 76 |
| ran but printed no report | 66 |
| crashed / refused | 21 |

Every module reads 0 / n, so this is not one construct in one module: the CCVS85
boilerplate that **every** program shares does not run to its end. That is the
single highest-value thing left in this folder, and it is now measured instead
of suspected.

Two things the harness needed, both learned the hard way:

- **An output cap, not just a timeout.** `IF101A` compiles clean and then loops
  writing to its print file — **4.2 GB in ten minutes** on the first attempt,
  which nearly filled the disk. The size is checked on the same tick as the
  clock.
- **The report is not line-delimited.** CCVS declares `PRINT-REC PIC X(120)` on
  a record-sequential file, so RustCOBOL writes fixed 120-byte records with no
  newline between them and the whole report is one very long line. A line-based
  scorer read it as a single record and scored nothing.

## What the 2026-08-27 pass learned

Working the failures by **root cause** rather than by module found that the two
largest categories were not language gaps at all:

- **33 programs on one optional word.** `[AT] END` — the `AT` is optional in
  COBOL-85, the bare form was not consumed, and the phrase then swallowed the
  next paragraph header. Every `GO TO` targeting that paragraph reported it
  undeclared, which is why the failures *looked* like a procedure-visibility
  problem spread across SQ, ST and IX.
- **4 programs on a literal that ran past its line** in the COPY preprocessor —
  the same defect the lexer fixed in 1.62.12, still present in the other half of
  the front end. It made the copyright banner's `COPY,` a directive.

Neither was visible from the module scoreboard, and neither was on the spec
list. **Re-derive the root-cause census after every fix**; the ranking changes
completely, and the biggest item is rarely the one with a spec.
