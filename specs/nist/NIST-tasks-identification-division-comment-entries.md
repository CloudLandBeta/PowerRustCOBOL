# Tasks — IDENTIFICATION DIVISION comment-entry paragraphs

- **Status:** draft → in progress → done
- **Spec:** [`NIST-spec-identification-division-comment-entries.md`](NIST-spec-identification-division-comment-entries.md)
- **Plan:** [`NIST-plan-identification-division-comment-entries.md`](NIST-plan-identification-division-comment-entries.md)
  **Date:** 2026-08-25

Same shape as the numeric-literals run, which worked: guards first, then the
change, then measure. T1-T2 change no behaviour; T3-T5 are the change.

> **Run test commands serially.** Concurrent `cargo test` jobs deadlock here —
> `cobolt-compiler` and `cobolt-runtime` tests spawn nested `cargo build` calls
> that block on the same `target/` lock. Check
> `ps -o etime=,command= -ax | grep 'cargo test'` first.

> **Blocked on Q3** (plan §8) — what to do about `REMARKS`, which NIST does not
> exercise at all. T3 needs the answer. Default if unanswered: accept and
> discard.

---

- [x] **T1 — A line-start helper on the parser** (no behaviour change)
  - Files: `crates/cobolt-parser/src/parser.rs`
  - Do: the termination rule needs "does this token begin a new source line?".
    `Span` carries `line` (`span.rs:23`) and `Parser` already exposes
    `tokens` / `pos` as `pub(crate)`, so add a small helper —
    `fn at_line_start(&self, offset: usize) -> bool` — returning true when the
    token at `offset` has a greater `line` than the token before it (and for the
    very first token). No caller yet.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast` — unchanged, and
    `cargo build -p cobolt-parser` warns about nothing new. (If dead-code warns,
    wire it in T3 in the same commit rather than adding `#[allow]`.)

- [x] **T2 — Pin what must NOT change** (AC6, plan risk table)
  - Files: `crates/cobolt-parser/tests/test_identification.rs`
  - Do: write these **before** T3 and confirm they pass against today's code.
    1. A program with **no** optional paragraphs parses; `author` is `None`
       *(AC6)*.
    2. `AUTHOR.` + one line of text still parses, and `author` is `Some` — the
       existing behaviour, which the RAD generator's banner depends on.
    3. `01 SECURITY PIC X(4).` in WORKING-STORAGE plus
       `MOVE "ABCD" TO SECURITY.` compiles — the word is **not** reserved, and
       must not become reserved (plan §4).
    4. A program that is only an IDENTIFICATION DIVISION (no ENVIRONMENT / DATA
       / PROCEDURE) — establishes today's behaviour before `Eof` termination
       is introduced. Record what it does now; if it already errors, that is the
       baseline, not a regression to fix.
  - Verify: `cargo test -p cobolt-parser --test test_identification` — 1-3 green
    on unmodified code. If (3) fails today, stop and report: the plan's
    "not reserved" assumption is wrong.

- [x] **T3 — The termination rule** (R2, R6)
  - Files: `crates/cobolt-parser/src/identification.rs`
  - Do: replace `collect_comment_text`'s period-based loop
    (`identification.rs:110`). Consume tokens until a token that **begins a new
    line** (T1's helper) **and** has header shape:
    - `Token::Author` / `DateWritten` / `DateCompiled`;
    - `Token::Identifier(s)` where `s` uppercases to `INSTALLATION`, `SECURITY`
      or `REMARKS`, **and the next token is a `Period`**;
    - `Token::Environment` / `Data` / `Procedure`, **and the next token is
      `Token::Division`**;
    - `Token::Identification`; `Token::Eof`.
    Both halves are required — line-start alone still breaks on a comment line
    that begins with `DATA`.
    Per Q3, add `REMARKS` to the accepted paragraph names (accept and discard;
    no AST field — see plan §3).
  - Verify: `cargo test -p cobolt-parser --no-fail-fast`, T2's guards still
    green, and:
    - CM101M's **nine-line** `INSTALLATION` (verbatim from the suite) parses,
      and the captured text contains both `GENERAL SERVICES` **and**
      `FALLS CHURCH` — proving it did not stop at the first period *(R2)*
    - a `SECURITY` entry containing `DATA`, `PROCEDURE`, `ENVIRONMENT` and
      `DIVISION` as prose parses, and the DATA DIVISION after it is still
      found *(AC3, R2)*

- [x] **T4 — The missing paragraphs** (R1, R5)
  - Files: `crates/cobolt-parser/src/identification.rs`
  - Do: add a `Token::DateCompiled` arm to the paragraph loop
    (`identification.rs:73`) — its absence is why it falls through `_ => break`.
    Make the `Identifier` arm recognise `INSTALLATION` / `SECURITY` / `REMARKS`
    by name rather than treating every identifier as a paragraph. Accept the
    paragraphs in **any order and any subset**.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast`
    - `DATE-COMPILED.  22ND AUG 1988.` parses *(AC1's construct)*
    - all six paragraphs in scrambled order each land correctly *(R5)*
    - NC303M parses clean *(AC1)*
    - EXEC85's two-line quoted `INSTALLATION` parses, both lines captured *(AC2)*

- [x] **T5 — Populate the AST fields** (R7, AC4)
  - Files: `crates/cobolt-parser/src/identification.rs`
  - Do: `installation`, `date_compiled` and `security` are hard-coded `None`
    (`identification.rs:100`). Assign the collected text. **No struct change** —
    the fields already exist (plan §3); adding one would alter the bincode
    format that `cobolt-compiler` embeds in built binaries.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast` — a program with all
    three paragraphs yields `Some(...)` in each, with the text verbatim *(AC4)*.

- [x] **T6 — The generated-code guard** (plan risk table)
  - Files: `crates/cobolt-parser/tests/test_identification.rs`
  - Do: the RAD generator emits `AUTHOR` in every generated form `.cbl`, so this
    code runs on the generated path. Compile a representative generated-form
    source (or a faithful excerpt of its banner) and assert it parses.
  - Verify: `cargo test -p cobolt-parser --test test_identification` **and**
    `cargo test -p cobolt-forms --features render --no-fail-fast` green.

- [x] **T7 — Re-measure the NIST suite** (AC5)
  - Files: none (measurement)
  - Do:
    ```bash
    cargo run -p cobolt-semantic --example nist_conformance -- strict
    ```
    Record PASS / FAIL / N-A and re-read the root-cause census.
  - Verify: the 32-program `expected Division, found …` bucket and the
    6-program `expected PROCEDURE DIVISION` bucket both shrink substantially.
    ⚠️ **Expect an in-scope gain of roughly 8-15, not 38.** Nine of the 32 are
    CM programs, which are N/A — they will start parsing and stay N/A. Report
    the number measured, never the bucket size.

- [x] **T8 — Docs** (English canonical only)
  - Files: `docs/cobol85-supported-syntax-en.md`
  - Do: two edits.
    1. The six IDENTIFICATION paragraphs are **not documented at all**. Add
       them, stating that a comment-entry is free text — it may contain
       reserved words and periods and spans lines until the next Area A entry —
       and that `REMARKS` is accepted for COBOL-74 compatibility but not stored.
    2. Update the **NIST scoreboard**: PASS/FAIL/N-A, per-module, the ranked
       root-cause table, and a conformance-history row.
       (`specs/steering/docs.md` requires this on every `specs/nist/` fix.)
  - Verify: numbers match T7 exactly; `iconv -f UTF-8 -t UTF-8` clean; every
    relative link resolves. **No translation file touched** — none exists for
    this document.
  - **i18n: nothing to do.** No user-facing IDE string is added; parser
    diagnostics are compiler output, not `Tr` fields. Confirm with
    `cargo test -p cobolt-ide --bin cobolt-ide i18n`.

- [x] **T9 — Update the specs**
  - Files: `specs/nist/NIST-spec-identification-division-comment-entries.md`,
    `NIST-spec-harness-and-baseline.md`, `README.md`
  - Do: mark the spec implemented with the measured result; tick AC1-AC6;
    refresh §6a of the baseline spec; re-rank the README (this change dethrones
    the current top bucket, so separators likely becomes largest).
  - Verify: `grep -oh 'NIST-\(spec\|plan\|tasks\)-[a-z-]*\.md' specs/nist/*.md |
    sort -u` — every reference resolves.

- [x] **T10 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the **fix number `z`** only (1.62.10 → 1.62.11; never the minor).
    Check the CHANGELOG head first — the operator has been landing entries
    concurrently, and 1.62.9 collided during the last run. Add a dated entry
    describing the defect and the measured before/after.
  - Verify — serially, reading every `test result:` line:
    ```bash
    cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic --no-fail-fast
    cargo test -p cobolt-runtime --no-fail-fast
    cargo test -p cobolt-forms --features render --no-fail-fast
    ```
    `cobolt-forms` needs `--features render` or it will not compile.
    Environmental, not caused by this change: `cobolt-compiler`'s
    `test_external_crates_e2e` (`libsqlite3-sys`) and
    `external_crates_service`'s live-network tests.
  - **Do not commit or push.** Classification: **fix** → forum **f=97** when
    merged to `main`.

---

## Coverage of the spec's acceptance criteria

| AC | Covered by |
|---|---|
| AC1 NC303M parses | T4 |
| AC2 EXEC85 two-line `INSTALLATION` | T4 |
| AC3 `SECURITY` containing `DATA` etc. | T3 |
| AC4 fields are `Some(...)` | T5 |
| AC5 buckets shrink | T7 |
| AC6 no optional paragraphs still parses | **T2**, before the change |

## Open question — must be answered before T3

**Q3 — `REMARKS`.** NIST contains **zero** `REMARKS` paragraphs (the spec's
count of 1 matched `VALUE "REMARKS"` inside a literal), so the suite does not
decide this. Options: drop from scope · accept and discard · reject as
non-COBOL-85. Recommendation and default: **accept and discard** — free, cannot
break anything, and courteous to COBOL-74 migrations.

## Done criteria

All six acceptance criteria ticked, the full sweep green with the two
environmental failures named explicitly rather than merely absent, docs and
specs carrying **re-measured** numbers, version and CHANGELOG done. Nothing
committed or pushed unless the operator asks.
