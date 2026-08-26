# Tasks — R6: a literal must not cross a line boundary

- **Status:** draft → in progress → done
- **Spec:** [`NIST-spec-literal-continuation.md`](NIST-spec-literal-continuation.md)
  (R6 and AC3/AC6, left open when the rest shipped at 1.62.8)
- **Plan:** [`NIST-plan-literal-continuation-r6.md`](NIST-plan-literal-continuation-r6.md)
  **Date:** 2026-08-25

Guards first, then the change, then measure — the shape that has worked for the
last three. One difference: **T2's tests are expected to FAIL before T3.** They
define the defect, so a green T2 means the diagnosis is wrong.

> **Serially.** Concurrent `cargo test` jobs deadlock here.
> **Keep failure names** — a tally-only summary discarded them during the
> 1.62.11 sweep and cost an unattributable forms failure:
> ```bash
> cargo test -p <crate> --no-fail-fast 2>&1 | tee /tmp/sweep.log \
>   | grep -E '^test result:|^test .* FAILED|^---- '
> ```

---

- [x] **T1 — Pin the continuation behaviour that must survive** (AC1, AC4, AC7)
  - Files: `crates/cobolt-lexer/src/source.rs` (tests), `crates/cobolt-lexer/tests/test_literals.rs`
  - Do: these already pass; the point is that they must **still** pass after T3,
    because a literal spanning lines is exactly what continuation produces —
    before the preprocessor joins it.
    1. NC113M's `HYPHEN-LINE` reassembles to exactly 54 hyphens
       (`strict_joins_a_continued_alphanumeric_literal` — already present) *(AC1)*.
    2. A continued literal padded to column 72 (`strict_pads_a_short_continued_line_to_column_72`).
    3. A continued **word** — NC205A's `DIVISION PICTURE X.` shape *(AC4)*.
    4. `X"0D0A"` still lexes to `StringLiteral("\r\n")` — the hex rule is
       separate and must not be caught by the newline exclusion.
    5. A doubled `""` escape inside a one-line literal.
  - Verify: `cargo test -p cobolt-lexer --no-fail-fast` — all green **now**.
    Record the count; T3 must not lower it.

- [x] **T2 — Write the containment tests (expected RED)** (R6, AC3)
  - Files: `crates/cobolt-lexer/tests/test_literals.rs`
  - Do: these describe the defect and must **fail** against today's code.
    1. `MOVE "abc TO X.` on line 1, `MOVE "def" TO Y.` on line 2 → line 2's
       literal is `def`. Today the stray quote pairs with line 2's opening
       quote and everything shifts.
    2. **The parity case, which is the real defect.** Four lines: prose
       containing `COMPILER"S`, two ordinary lines, then a balanced `"pair"`.
       Assert `"pair"` survives intact. Today it does not — measured: the
       failing NIST programs have *even* quote counts, so this is mis-pairing
       that cascades, not an unterminated literal running to EOF.
    3. Same containment for `'` single quotes.
  - Verify: `cargo test -p cobolt-lexer --test test_literals` — the three
    **FAIL**. If any passes now, stop and report: the diagnosis in the plan is
    wrong and the regex is not the cause.

- [x] **T3 — Confine a literal to its line** (R6)
  - Files: `crates/cobolt-lexer/src/token.rs`
  - Do: in the two string rules, exclude the newline from the body class —
    `[^"\\]` → `[^"\\\n]`, `[^'\\]` → `[^'\\\n]`. (`\\.` is already
    newline-safe: Rust's `.` does not match `\n`.) Leave the hex rules alone.
  - Verify: `cargo test -p cobolt-lexer --no-fail-fast` — **T2's three now pass,
    T1's five still pass**, and the total is no lower than T1's recorded count.

- [x] **T4 — Say what went wrong** (R6 reporting half, AC3)
  - Files: `crates/cobolt-lexer/src/lexer.rs`, `crates/cobolt-parser/src/…`
  - Do: after T3 an unpaired quote matches no rule, so logos yields
    `Token::Error("\"")`. Two things:
    1. Construct `LexError::UnterminatedString { span }` when the unmatched
       character is `"` or `'`. The variant exists at `lexer.rs:59` and **is
       currently never produced** — this is its first caller.
    2. Make the parser name the cause when a `Token::Error` holding a quote
       reaches a statement, instead of the generic unexpected-token message.
       Wording per plan Q1; default:
       *"unterminated alphanumeric literal — a literal cannot span source
       lines; use a continuation line (`-` in column 7) in fixed format"*.
  - Verify: `cargo test -p cobolt-parser --no-fail-fast` — an unterminated
    literal in a statement produces **one** diagnostic, on its own line, and the
    **following statement still parses** *(AC3)*.
  - ⚠️ Do **not** change `tokenize()`'s signature. It drops `Lexer::errors`
    today and has **64 product call sites**; the diagnostic travels via
    `Token::Error`, which is the path that already works (`data.rs:500` relies
    on it for the `$` currency symbol).

- [x] **T5 — The `EXEC RUST` guard** (AC7, plan risk 1)
  - Files: `crates/cobolt-parser/tests/test_exec_rust.rs`
  - Do: Rust genuinely has multi-line strings, and generated form `.cbl` files
    carry `EXEC RUST` blocks, so this is the main exposure. The block is
    captured by **offset slicing** between `RUST` and `END-EXEC`
    (`lexer.rs:390`) and the scan only matches `Word("END-EXEC")`, so error
    tokens in between are skipped — but pin it:
    1. A block whose Rust body contains a multi-line string round-trips with
       the captured source byte-identical to before.
    2. A block whose Rust body contains a **single-line** string holding the
       text `END-EXEC` still runs to the real terminator.
  - Verify: `cargo test -p cobolt-parser --test test_exec_rust --no-fail-fast`,
    then `cargo test -p cobolt-forms --features render --no-fail-fast` *(AC7)*.
  - **Known residual:** a *multi-line* Rust string containing the characters
    `END-EXEC` would now terminate the block early. Vanishingly unlikely; if
    (2) is easy to extend to the multi-line case, assert the current behaviour
    rather than pretending it is fixed.

- [x] **T6 — Close AC6: the value, not the parse** (AC2, AC6)
  - Files: `crates/cobolt-runtime/tests/test_decimal_point_values.rs` *(extend)*
    or a new `test_continuation_values.rs`
  - Do: AC6 is the other item left open at 1.62.8 — *"a continued literal whose
    stray quotes happen to balance produces the correct value, verified by
    executing the program"*. A clean parse proves nothing here (baseline spec
    R8). Run a fixed-format program with a continued literal and `DISPLAY` it:
    - NC113M's `HYPHEN-LINE` → exactly 54 hyphens *(AC1's value half)*
    - `p5_continuation.cbl`'s `RE-MARK` → `PRESENT INCORRECT` *(AC2's value half)*
  - Verify: `cargo test -p cobolt-runtime --test <name> --no-fail-fast`.
  - **Beyond the plan's §1**, which covered only R6. Included because leaving
    one acceptance criterion open when it is a short runtime test is worse than
    the small scope increase — but it is a scope call, so say so in the report.

- [x] **T7 — Re-measure the NIST suite** (AC5)
  - Do: `cargo run -p cobolt-semantic --example nist_conformance -- strict`
  - Verify: the 6-program `expected PROCEDURE DIVISION` bucket (NC215A, SG104A,
    SG105A, SG106A and peers) clears, and Segmentation moves off **0 / 13**.
    ⚠️ **Report the measured gain, not the bucket size.** Some of these
    programs will meet segment priority numbers next
    ([`NIST-spec-segmentation.md`](NIST-spec-segmentation.md)), so the in-scope
    gain may be well under 6 — that is the 1.62.11 lesson.

- [x] **T8 — Docs** (English canonical only)
  - Files: `docs/cobol85-supported-syntax-en.md`
  - Do: two edits.
    1. **Replace the stale warning.** 1.62.11 added a ⚠️ note under
       *IDENTIFICATION DIVISION paragraphs* telling developers to avoid an
       unpaired quote in a comment-entry. Once this lands that advice misleads —
       replace it with the rule that a literal cannot span a line, and that an
       unpaired quote is now reported where it is written.
    2. Update the **NIST scoreboard**: PASS/FAIL/N-A, per-module, the ranked
       root-cause table, and a conformance-history row
       (`specs/steering/docs.md` requires this on every `specs/nist/` fix).
  - Verify: numbers match T7 exactly; `iconv -f UTF-8 -t UTF-8` clean; links
    resolve. No translation file touched — none exists for this document.
  - **i18n: nothing to do.** No user-facing IDE string; lexer and parser
    diagnostics are compiler output, not `Tr` fields. Confirm with
    `cargo test -p cobolt-ide --bin cobolt-ide i18n`.

- [x] **T9 — Close the spec properly**
  - Files: `specs/nist/NIST-spec-literal-continuation.md`,
    `NIST-spec-harness-and-baseline.md`, `README.md`
  - Do: **all seven acceptance criteria are still unticked**, including the five
    satisfied back at 1.62.8. Tick each with the evidence that satisfies it
    (AC1 → `strict_joins_a_continued_alphanumeric_literal`; AC5 → the 48 → 6
    bucket drop; AC7 → the forms suite; AC3/AC6 → T4/T6). Remove the
    "Still open" note from the status block. Refresh the baseline §6a and the
    README ranking.
  - Verify: no unticked `- [ ] AC` remains in that spec;
    `grep -oh 'NIST-\(spec\|plan\|tasks\)-[a-z0-9-]*\.md' specs/nist/*.md |
    sort -u` all resolve.

- [x] **T10 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the **fix number `z`** only (1.62.11 → 1.62.12; never the minor).
    **Check the CHANGELOG head first** — the operator has landed entries
    concurrently twice now. Describe the defect as what it is: one stray quote
    flipping the parity of a whole file, not an unterminated literal.
  - Verify — serially, **keeping failure names**:
    ```bash
    cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic --no-fail-fast 2>&1 \
      | tee /tmp/s1.log | grep -E '^test result:|^test .* FAILED|^---- '
    cargo test -p cobolt-runtime --no-fail-fast 2>&1 \
      | tee /tmp/s2.log | grep -E '^test result:|^test .* FAILED|^---- '
    cargo test -p cobolt-forms --features render --no-fail-fast 2>&1 \
      | tee /tmp/s3.log | grep -E '^test result:|^test .* FAILED|^---- '
    ```
    Environmental, not this change: `cobolt-compiler`'s
    `test_external_crates_e2e` (`libsqlite3-sys`, a C build), the
    `external_crates_service` live-network tests, and **`cobolt-forms`'
    `test_map_tiles_tls.rs`**, which fetches a real tile from
    `tile.openstreetmap.org` — that is what flaked in the 1.62.11 sweep.
  - **Do not commit or push.** Classification: **fix** → forum **f=97**.

---

## Coverage of the spec's acceptance criteria

| AC | Covered by |
|---|---|
| AC1 54 hyphens | T1 (parse), T6 (value) |
| AC2 `RE-MARK` value | T6 |
| AC3 one diagnostic, rest parses | **T4** |
| AC4 continued word | T1 |
| AC5 suite improves | T7 |
| AC6 value by execution | **T6** |
| AC7 generated forms unaffected | T5 |

## Open question — answer before T4

**Q1 — the diagnostic wording.** Proposal:
*"unterminated alphanumeric literal — a literal cannot span source lines; use a
continuation line (`-` in column 7) in fixed format"*. It names the cause and
the remedy, and the remedy is not obvious to someone writing free-format source.
T4 proceeds with this unless told otherwise.

Q2 (no diagnostic inside a comment-entry) and Q3 (the "raise an error" ruling
applies) are settled in the plan and need no decision.

## Done criteria

All seven acceptance criteria ticked with evidence, the full sweep green with
the environmental failures named rather than merely absent, docs and specs
carrying **re-measured** numbers, version and CHANGELOG done. Nothing committed
or pushed unless the operator asks.
