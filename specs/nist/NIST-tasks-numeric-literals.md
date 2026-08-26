# Tasks — numeric literals with a leading decimal point

- **Status:** draft → in progress → done
- **Spec:** [`NIST-spec-numeric-literals.md`](NIST-spec-numeric-literals.md)
- **Plan:** [`NIST-plan-numeric-literals.md`](NIST-plan-numeric-literals.md)
  **Date:** 2026-08-25

Ordered so the tree stays green after every task. T1-T2 change no behaviour;
T3 is the change; T4-T7 extend it; T8-T12 measure, document and finalize.

> **Run test commands serially.** Two concurrent `cargo test` jobs deadlock in
> this workspace: `cobolt-compiler` and `cobolt-runtime` tests spawn nested
> `cargo build` calls that block on the same `target/` lock. Check
> `ps -o etime=,command= -ax | grep 'cargo test'` before starting.

---

- [x] **T1 — Extract the span-adjacency helper** (no behaviour change)
  - Files: `crates/cobolt-parser/src/expr.rs`
  - Do: the decimal-comma code in `parse_literal_inner` (~line 88) already
    computes "are these tokens adjacent?" and "how many digits did the
    fractional token span?". Lift both into one private helper — something like
    `fn adjacent_digits(p: &Parser, at: usize) -> Option<(i128, u8)>` returning
    (value, digit-count) when the token at `at` is `IntegerLiteral` or
    `LevelNumber` **and** starts exactly where the previous token ended. Rewrite
    the existing comma paths to use it. Behaviour must be identical.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast` — every test that
    passed before still passes, in particular `test_decimal_comma.rs`. No new
    tests in this task.

- [x] **T2 — Pin the two things that must NOT change** (AC5, risk table)
  - Files: `crates/cobolt-parser/tests/test_decimal_point.rs` *(new)*
  - Do: write these tests **before** T3, and confirm they pass against today's
    code. They are the false-positive guards, and the highest-severity risk in
    the plan is that T3 breaks one of them.
    1. `MOVE X TO Y.` followed by another sentence → two statements, the period
       still terminates.
    2. `77 N PIC 9 VALUE 1.` → `Literal::Integer(1)`, period terminates the entry.
    3. `01 W PIC .9999/99999,99999,99.` → PICTURE template is **exactly**
       `.9999/99999,99999,99` (this is CCVS85's `WRK-NE-1`, 9 programs).
    4. `MOVE X TO Y. 5 …` — a space after the period → still a terminator.
  - Verify: `cargo test -p cobolt-parser test_decimal_point` — **all four green
    on unmodified code.** If any fails now, stop: the assumption behind the plan
    is wrong.

- [x] **T3 — Leading decimal point in `parse_literal_inner`** (R1, R5, R6)
  - Files: `crates/cobolt-parser/src/expr.rs`
  - Do: add a `Token::Period` arm using the T1 helper. Fire only when the next
    token is `IntegerLiteral(_)` **or** `LevelNumber(_)` *and* is span-adjacent
    to the period. Build `Literal::Decimal(mantissa, scale)` where `mantissa` is
    the token's value and `scale` is the span width — the width is what
    preserves leading zeros, which the parsed value has already lost.
    `LevelNumber` must be accepted: `Period` sets the lexer's `at_line_start`,
    so `.1` → `LevelNumber(1)` and `.09` → `LevelNumber(9)`.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast`, and **T2's four guards
    still green**. Add and assert:
    - `MOVE .00001 TO X.` → `Decimal(1, 5)` *(AC1)*
    - `COMPUTE N = FUNCTION ACOS(.999).` → argument `Decimal(999, 3)` *(AC2)*
    - `77 A PIC SV9(5) VALUE .11111.` → `Decimal(11111, 5)` *(AC3)*
    - `77 B PIC SP(8)9 VALUE .000000001.` → `Decimal(1, 9)` *(AC8, parse half)*
    - `IF X = .1` → `Decimal(1, 1)` — the `LevelNumber` path *(AC4)*
    Assert mantissa **and** scale in every case; a test that only checks "it
    parsed" would pass with the wrong value.

- [x] **T4 — Signed leading decimals** (R2)
  - Files: `crates/cobolt-parser/tests/test_decimal_point.rs`
  - Do: confirm the existing sign folding covers `-.5` / `+.5` in both an
    expression (`COMPUTE X = -.5`) and a `VALUE` clause (`VALUE -.5`). The lexer
    emits the sign separately by design, and `data.rs:275` and `parse_primary`
    already fold it — if both work, this task is tests only. If `VALUE -.5`
    fails, fix the fold, not the lexer.
  - Verify: `cargo test -p cobolt-parser test_decimal_point`

- [x] **T5 — Decimal-comma mirror** (R4, AC6)
  - Files: `crates/cobolt-parser/src/expr.rs`, `tests/test_decimal_point.rs`
  - Do: extend the T3 arm to `Token::Comma` when `p.decimal_comma` is set, using
    the same helper. Roles swap: comma is the point.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast`
    - `VALUE ,11111` under `DECIMAL-POINT IS COMMA` → `Decimal(11111, 5)` *(AC6)*
    - **`VALUE 8,49` without the clause still produces the existing diagnostic,
      verbatim** — that message names the file and the fix and must not regress.
    - A separator comma with a space (`MOVE ZERO TO A, B`) is untouched.

- [x] **T6 — The remaining literal sites** (R1)
  - Files: `crates/cobolt-parser/tests/test_decimal_point.rs`
  - Do: `parse_literal` has 8 callers beyond `parse_primary`. Cover the ones
    where a leading-dot literal is legal COBOL — at minimum the 88-level path
    (`data.rs:875`): `88 C VALUE .5 THRU .9.`; and `EVALUATE … WHEN .5`.
  - Verify: `cargo test -p cobolt-parser test_decimal_point`

- [x] **T7 — Prove the value, not the parse** (AC8, AC1, AC3)
  - Files: `crates/cobolt-runtime/tests/test_decimal_point_values.rs` *(new)*
  - Do: spec §6 warns that the DATA DIVISION already accepts
    `VALUE .11111` **without an error while mis-reading it**, so a clean parse
    proves nothing (`NIST-spec-harness-and-baseline.md` R8). Run a program that
    `DISPLAY`s results computed from `.11111`, `.000000001` and `.1`, and assert
    the printed text.
  - Verify: `cargo test -p cobolt-runtime test_decimal_point_values`

- [x] **T8 — A CCVS-style COBOL test with quantified output** (GOLDEN RULE #7)
  - Files: `tests/cobol/numeric/leading-decimal-point.cbl` *(new)*, driven from
    the T7 test file
  - Do: exercise each literal form and print **one summary block at the end** —
    a table of `form → expected → actual → PASS/FAIL` plus the tally. Not a bare
    count; the reader must finish knowing which forms ran.
  - Verify: `cargo run -p cobolt-cli -- run tests/cobol/numeric/leading-decimal-point.cbl`
    prints the summary and reports every case PASS.

- [x] **T9 — Re-measure the NIST suite** (AC7)
  - Files: none (measurement)
  - Do:
    ```bash
    cargo run -p cobolt-semantic --example nist_conformance -- strict
    ```
    Record PASS / FAIL / N-A and re-read the **root-cause census**, which
    reorders after every fix.
  - Verify: PASS rises from **222 / 434**; the `expected expression, found …`
    bucket (**36** programs today — the spec's "37" predates the source-format
    fix) is empty or much smaller; IF improves from 21 / 45.
    ⚠️ Do **not** attribute the whole IF delta to this task — `FUNCTION x(ALL)`
    (13 programs) sits behind this gap in some of the same programs.

- [x] **T10 — Docs** (English canonical only)
  - Files: `docs/cobol85-supported-syntax-en.md`
  - Do: two edits.
    1. The literals bullet says "Literals: integer, decimal, …" — state that a
       numeric literal **may begin with a decimal point** (`.5`, `-.5`), and
       that a literal may not *end* with one.
    2. Update the **NIST scoreboard** — the PASS/FAIL/N-A table, the per-module
       table, the ranked root-cause table, and add a row to the conformance
       history. `specs/steering/docs.md` now requires this on every
       `specs/nist/` fix.
  - Verify: numbers match T9's output exactly; `iconv -f UTF-8 -t UTF-8` clean;
    all relative links resolve. **No translation file is touched** (none exists
    for this document, so GOLDEN RULE #8's deletion step is moot).
  - **i18n: nothing to do.** This change adds no user-facing IDE string — parser
    diagnostics are compiler output, not `Tr` fields. Confirm with
    `cargo test -p cobolt-ide --bin cobolt-ide i18n` (still green, no new keys).

- [x] **T11 — Update the specs**
  - Files: `specs/nist/NIST-spec-numeric-literals.md`,
    `NIST-spec-harness-and-baseline.md`, `README.md`
  - Do: mark the spec ✅ IMPLEMENTED with the measured result; tick AC1-AC8;
    refresh §6a of the baseline spec (current numbers + new census ranking); fix
    the README's "largest bucket" ranking, which this change dethrones.
  - Verify: `grep -oh 'NIST-spec-[a-z-]*\.md' specs/nist/*.md | sort -u` — every
    reference resolves to a file that exists.

- [ ] **T12 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the **fix number `z`** only (`1.62.8` → `1.62.9`; never the minor —
    only the operator raises `x`/`y`). Add a top-of-file CHANGELOG entry dated
    2026-08-25, describing the defect and the measured before/after.
  - Verify — full sweep, **serially**, reading every `test result:` line rather
    than grepping for failures:
    ```bash
    cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic --no-fail-fast
    cargo test -p cobolt-runtime --no-fail-fast
    cargo test -p cobolt-forms --features render --no-fail-fast
    ```
    `cobolt-forms` needs `--features render` or it will not compile.
    Known-environmental, not caused by this change: `cobolt-compiler`'s
    `test_external_crates_e2e` (`libsqlite3-sys`, a C build) and
    `external_crates_service`'s live-network tests.
  - **Do not commit or push.** The São Paulo work-hours embargo applies, and the
    tree also holds the operator's in-flight work. Classification: **fix** →
    forum **f=97** when merged to `main`, not f=96.

---

## Coverage of the spec's acceptance criteria

| AC | Covered by |
|---|---|
| AC1 `MOVE .00001` | T3, T7 |
| AC2 `FUNCTION ACOS(.999)` | T3 |
| AC3 `VALUE .11111` | T3 (parse), **T7 (value)** |
| AC4 `IF X = .1` | T3 |
| AC5 no regression | **T2** (written first, must pass before T3) |
| AC6 decimal comma | T5 |
| AC7 suite improves | T9 |
| AC8 exact scale, by execution | T7, T8 |

## Open question carried from the plan

**Q3 — warn on `MOVE X TO Y.5`?** A period adjacently followed by digits where
no literal is expected is a probable typo. Recommendation: **no** for this
change — new strictness no NIST program asks for, and T2's guards already prove
the meaning of existing code cannot change silently. Decide before T3; if the
answer is yes, it becomes T3b.

## Done criteria

All eight acceptance criteria ticked, the full sweep green (with the two
environmental failures named explicitly, not merely absent), docs and specs
updated with **re-measured** numbers, version and CHANGELOG done. Nothing
committed or pushed unless the operator asks.
