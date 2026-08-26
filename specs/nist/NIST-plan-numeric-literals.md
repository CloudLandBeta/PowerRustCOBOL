# Plan — numeric literals with a leading decimal point

- **Status:** draft → approved
- **Spec:** [`NIST-spec-numeric-literals.md`](NIST-spec-numeric-literals.md)
  **Date:** 2026-08-25
- **Impact:** 48 CCVS85 programs contain such a literal; **36 fail on it as
  their first error** — the largest single root-cause bucket at the current
  222 / 434 baseline.

## 1. Approach

**Do it in the parser, not the lexer, using span adjacency — because that
pattern already exists here and already solves the mirror-image problem.**

`crates/cobolt-parser/src/expr.rs:88` handles `DECIMAL-POINT IS COMMA`, where
`12345678,91` must read as one decimal literal. It does so by looking at three
consecutive tokens — `IntegerLiteral`, `Comma`, `IntegerLiteral` — and requiring
them to be **span-adjacent** (`comma_sp.start == int_end && frac_sp.start ==
comma_sp.end`), because COBOL-85 requires a space after a *separator* comma and
so adjacency is what distinguishes the two meanings. It recovers the fractional
scale from the span width (`frac_sp.end - frac_sp.start`), which preserves
leading zeros that the token's parsed value has already lost.

A leading decimal point is the same problem with the same solution. `.00001`
lexes today as `Period` followed by `LevelNumber(1)`; the token value gives the
mantissa (`1`) and the span width gives the scale (`5`), so the literal
reconstructs exactly as `0.00001`.

The whole change is **one new arm in `parse_literal_inner`** (R1, R5, R6), plus
sign folding that already exists (R2) and a mirrored comma arm (R4).

### Why not the lexer — this is the decision that matters

Adding `#[regex(r"\.[0-9]+")]` to `RawToken` looks simpler and is wrong: a
lexical rule cannot tell a numeric literal from a **numeric-edited PICTURE that
begins with a decimal point**. Both are "space, dot, digits". CCVS85 contains
both:

```cobol
77  A05ONES-DS-00V05  PICTURE SV9(5) VALUE .11111.   *> literal — 157 in CCVS85
01  WRK-NE-1          PIC .9999/99999,99999,99.      *> PICTURE — 9 in CCVS85
```

`parse_pic_clause` (`data.rs:443`) reassembles a PICTURE from raw tokens and has
its own `Token::Period` and `Token::DecimalLiteral` arms; a lexer-formed literal
would land in that loop and `decimal_to_pic(9999, 4)` would rebuild `"0.9999"`
instead of `".9999"`, silently corrupting the picture. Distinguishing the two in
the lexer would need PICTURE-context state.

Parsing it in `parse_literal_inner` avoids all of that for free: **the PICTURE
loop never calls `parse_literal`**, so PICTURE lexing and parsing stay
byte-identical, and the 9 programs above keep working without a single line
changed in the picture path.

### Coverage — one arm reaches every site

`parse_literal_inner` is the single choke point. `parse_literal` wraps it
(figurative constants first) and is called from `parse_primary`
(`expr.rs:252` — so every expression, including `FUNCTION` arguments and
`COMPUTE`) plus 8 other sites, including the DATA DIVISION `VALUE` clause
(`data.rs:279`) and the 88-level values path (`data.rs:875`). One arm therefore
satisfies:

| Requirement | Reached via |
|---|---|
| R1 `.5` after space / `(` / `,` / operator | `parse_literal_inner` |
| R2 optional `+` / `-` | existing sign folding — `data.rs:275` for `VALUE`, `parse_primary`'s unary handling for expressions |
| R4 decimal comma | mirrored `Token::Comma` arm, gated on `p.decimal_comma` |
| R5 exact scale, no rounding | `Literal::Decimal(mantissa, scale)` is already exact fixed-point (`i128` mantissa) |
| R6 no level-number misclassification | the digits are consumed by the literal arm before any level-number question arises |

### The adjacency rule

The new arm fires only when **all** hold:

1. current token is `Token::Period` (or `Token::Comma` under `decimal_comma`);
2. the next token is `Token::IntegerLiteral(_)` **or** `Token::LevelNumber(_)`;
3. `next_span.start == period_span.end` — no whitespace between them.

Condition 3 is what makes it safe. A sentence-ending period is always followed
by whitespace or a newline (in fixed format, `flatten_fixed_strict` emits a
newline per line), so `MOVE X TO Y.` can never be mistaken for the start of a
literal.

Condition 2 must accept **both** integer tokens. `Period` sets the lexer's
`at_line_start` (`lexer.rs:291`), so the digits after it are offered to
`keywords::is_level_number`: `.1` → `LevelNumber(1)`, `.09` → `LevelNumber(9)`,
`.999` → `IntegerLiteral(999)`. Accepting only one of the two would fix
`FUNCTION ACOS(.999)` and leave `IF X = .1` broken.

## 2. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-parser/src/expr.rs` | **the change** — new `Token::Period` arm in `parse_literal_inner` (~30 lines), plus the `Token::Comma` mirror for R4. Factor the shared adjacency + scale-from-span logic into one helper reused by the existing comma code. |
| `crates/cobolt-parser/tests/test_literals*.rs` *(new file, or extend `test_data_division.rs`)* | parser-level cases — see §6 |
| `crates/cobolt-runtime/tests/` | one execution test proving the **value**, not just the parse (spec AC8) |
| `docs/cobol85-supported-syntax-en.md` | the literals bullet says "Literals: integer, decimal, …" without stating a leading point is allowed; state it. Re-measure and update the **NIST scoreboard** — it is the document's headline and `specs/steering/docs.md` now requires it. |
| `specs/nist/NIST-spec-numeric-literals.md` | mark implemented, record the new PASS figure |
| `specs/nist/NIST-spec-harness-and-baseline.md`, `README.md` | new baseline + re-read census |
| `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` | `z` bump + entry |

**Not touched:** `crates/cobolt-lexer/**` (no token or regex change),
`parse_pic_clause`, `numedit.rs`.

## 3. Data / model changes

**None.** No new token variant, no AST change, no format change.
`Literal::Decimal(i128, u8)` already carries an exact fixed-point value, which
is what R5 needs for `A01ONE-DS-P0801 PICTURE SP(8)9 VALUE .000000001` (scale 9).

This is the main argument for the parser approach: the alternative needed a
`leading_point: bool` on `Token::DecimalLiteral`, touching its 9 product sites
and 6 test sites.

## 4. Key decisions & alternatives

- **Decision: parse-time, span-adjacency.** *Why:* the only approach that
  cannot break a leading-dot PICTURE, and it reuses a pattern already proven in
  this file for the decimal comma. *Rejected:* a lexer regex — corrupts
  `PIC .9999/99999,99999,99` (9 CCVS85 programs) with no diagnostic.
- **Decision: accept `LevelNumber` as well as `IntegerLiteral` after the point.**
  *Why:* measured — `Period` sets `at_line_start`, so `.1` and `.09` arrive as
  level numbers. *Rejected:* changing the lexer's level-number rule — it is
  correct for its own purpose and used by every data description entry.
- **Decision: recover scale from the span width, not from the token value.**
  *Why:* `.00001` parses to `1`; only the span knows there were five digits.
  *Rejected:* carrying digit text on `IntegerLiteral` — invasive, and the
  existing comma code already proves the span works.
- **Decision: implement R4 (comma) by mirroring, in the same change.** *Why:*
  the two are one rule with the roles swapped, and leaving them asymmetric is
  how the next reader gets confused. *Rejected:* deferring — it is ~10 lines
  once the helper exists.
- **Decision: no diagnostic for `5.` (trailing point).** *Why:* the standard
  forbids a literal ending in a decimal point, and `5.` is already correctly
  read as `5` + sentence terminator. Spec non-goal.

## 5. Risks & mitigations

| Risk | Mitigation |
|---|---|
| **A sentence-ending period is swallowed as a literal**, silently merging two statements. | Adjacency (condition 3). Add an explicit regression test for `MOVE X TO Y.` at end of line, and one for `MOVE X TO Y. 5 …` where a space intervenes. This is the highest-severity risk: a false positive would corrupt working programs. |
| **A leading-dot PICTURE regresses.** | Structurally impossible — `parse_pic_clause` does not call `parse_literal`. Pin it anyway with a test on `PIC .9999/99999,99999,99` asserting the exact template, and by re-running the 9 CCVS85 programs that use it. |
| **`88` condition-name `VALUE .5 THRU .9` mis-parses.** | `parse_88_values` calls `parse_literal`; add a case. |
| **Reference modification `X(1:.5)`** or other `(`-adjacent contexts behave oddly. | Not valid COBOL; adjacency plus the existing `:` disambiguation in `parse_primary` leaves it alone. Note only. |
| **The comma arm misfires** on `MOVE ZERO TO A,B` (no space). | The new arm only runs when `parse_literal` is called *and* `p.decimal_comma` is set; `A,B` is an identifier list and never reaches it. Existing behaviour for the undeclared case — the good "reads as a comma decimal separator" diagnostic — must be preserved verbatim; test it. |
| **The IF module improves less than expected**, because a second gap (`FUNCTION x(ALL)`, 13 programs) sits behind this one in the same programs. | Expected and fine. Re-read the census after landing; do not attribute the whole IF delta to this spec. |

## 6. Test strategy

**Parser tests** (`cobolt-parser`) — assert the parsed `Literal`, so scale is
checked, not just "it parsed":

| Case | Asserts |
|---|---|
| `MOVE .00001 TO X.` | `Decimal(1, 5)` — mantissa **and** scale |
| `COMPUTE N = FUNCTION ACOS(.999).` | argument is `Decimal(999, 3)` |
| `77 A PIC SV9(5) VALUE .11111.` | `Decimal(11111, 5)` |
| `77 B PIC SP(8)9 VALUE .000000001.` | `Decimal(1, 9)` — R5, the exact-scale case |
| `IF X = .1` | `Decimal(1, 1)` — the `LevelNumber` path |
| `COMPUTE X = -.5` / `+.5` | R2, both signs |
| `88 C VALUE .5 THRU .9.` | 88-level path |
| **`MOVE X TO Y.`** | still one statement, period still terminates — the false-positive guard |
| **`01 W PIC .9999/99999,99999,99.`** | template is exactly `.9999/99999,99999,99` — the PICTURE guard |
| `VALUE ,11111` under `DECIMAL-POINT IS COMMA` | R4 |
| `VALUE 8,49` without the clause | the existing diagnostic is unchanged |

**Runtime test** (`cobolt-runtime`) — spec AC8 requires the *value*, because a
clean parse is not proof (`NIST-spec-harness-and-baseline.md` R8). One program
that `DISPLAY`s the result of arithmetic on `.000000001` and `.11111`, checked
against expected text.

**GOLDEN RULE #7** — if a `tests/cobol/` program is added it must print a
quantified summary: the list of literal forms exercised and an input→expected→
actual table, not a bare pass count.

**Suite-level** — the real verdict:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

Expect **PASS to rise from 222 / 434**, the 36-program
`expected expression, found …` root-cause bucket to empty, and IF to improve
from 21 / 45 (partially — see the risk table). Record the new figures in the
doc scoreboard and the baseline spec.

**Regression** — `cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic
-p cobolt-runtime --no-fail-fast`, reading every `test result:` line rather than
grepping for failures. `cobolt-forms` needs `--features render`. Run them
**serially**: concurrent `cargo test` jobs deadlock here, because
`cobolt-compiler`/`cobolt-runtime` tests spawn nested `cargo build` calls that
block on the same `target/` lock.

## 7. Steering compliance

- [x] **i18n** — no new UI strings; parser diagnostics are compiler output, not
      `Tr` fields. Nothing to translate.
- [x] **Generated-code contract** — the RAD generator emits integer literals and
      `VALUE ZERO`; the change is additive and cannot alter them. Forms suite
      (`--features render`) must stay green.
- [x] **Docs** — English canonical only. `cobol85-supported-syntax-en.md`
      (literals bullet **and** the NIST scoreboard). No translations of that
      file exist, so GOLDEN RULE #8's deletion step is moot. The Developer's
      Guide needs no change: a leading decimal point is not a new capability a
      PowerCOBOL/isCOBOL developer must learn, it is one they already assume.
- [x] **System KB** — no control, property, method or event changes, so
      `assets/knowledge/chunked.data` does not need rebuilding. Confirm the
      freshness test stays green during `/implement`.
- [x] **Fix vs feature** — **fix** (missing COBOL-85 standard support =
      technical debt, CLAUDE.md rule #4). Bump `z` in
      `crates/cobolt-ide/src/version.rs`, add a `CHANGELOG.md` entry, announce
      on forum **f=97** when merged to `main` — not f=96.
- [x] **No "cobolt" in user-facing text; COBOL identifiers English** — unaffected.
- [x] **Branch** — `fixes` (already current).

## 8. Open questions

Both spec open questions are resolved by the design above:

- **Q1** (does `parse_decimal_token` handle a missing integer part?) — moot: the
  lexer is not touched, and the parser builds the literal from mantissa + span
  width.
- **Q2** (which characters may precede the point?) — the adjacency rule replaces
  the character list entirely. What precedes the period is irrelevant; only the
  absence of a gap *after* it matters. Simpler and safer than enumerating
  `=`, `(`, `,` and the operators.

One new question for `/tasks`:

- **Q3** — should a `WARNING` be emitted when a period is followed adjacently by
  digits in a context where a literal is *not* expected (a probable typo, e.g.
  `MOVE X TO Y.5`)? Recommendation: **no** for this change. It would be new
  strictness with no NIST program asking for it, and the risk table's guard
  tests already prove the case cannot silently change meaning.
