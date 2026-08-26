# Plan — R6: a literal must not cross a line boundary

- **Status:** draft → approved
- **Spec:** [`NIST-spec-literal-continuation.md`](NIST-spec-literal-continuation.md)
  **R6**, left open when that spec shipped at 1.62.8
  **Date:** 2026-08-25
- **Impact:** 6 programs fail on this as their *first* error (NC215A, SG104A,
  SG105A, SG106A and peers). It is the sole reason the **Segmentation module
  reads 0 / 13** — those programs never reach their segment numbers.
- **Surfaced by:** `NIST-spec-identification-division-comment-entries.md` §10.
  Fixing the comment-entry rule let these programs read further into their own
  source, where they met this.

## 1. Approach

**One character class in two regexes.** COBOL-85 has no multi-line literal:
continuation is a *source-format* mechanism, resolved before lexing. So the
lexer's string rules should not be able to cross a newline, and today they can:

```rust
// crates/cobolt-lexer/src/token.rs
#[regex(r#""([^"\\]|\\.)*""#, …)]  StringDouble(String),
#[regex(r"'([^'\\]|\\.)*'", …)]    StringSingle(String),
```

`[^"\\]` matches a newline, so an unpaired quotation mark consumes lines until
it finds another quote. Excluding `\n` from that class makes a literal a
single-line construct, which is what the standard says it is. (`\\.` is already
newline-safe — Rust's `.` does not match `\n` by default.)

### What this actually fixes — and the measurement that reframed it

The failing programs contain a quotation mark inside ordinary prose:

```
004100     THIS PROGRAM CHECKS THE COMPILER"S ABILITY TO HANDLE EIGHT
```

The obvious reading is "an unterminated literal runs to end of file". **That is
not what happens.** Measured: SG104A, SG105A, SG106A and NC215A each contain an
**even** number of quotation marks — 194, 194, 194 and 224. The quotes balance.

What actually happens is worse and more interesting: `COMPILER"S` opens a
literal that closes at the *next* quotation mark, several lines later. That
swallows everything between — including the `ENVIRONMENT DIVISION` header — and
every subsequent quote in the file is then paired with the wrong partner. One
stray character shifts the parity of the entire program.

Confining a literal to its line makes the damage **self-correcting at the next
newline**: the offending line yields an error token, and the line after it pairs
correctly again. That is the real value of R6, and it is a stronger argument
than "report unterminated literals".

### R6's second half — reporting

With the regex change, a lone quotation mark no longer matches any rule, so
logos yields `Token::Error("\"")` at that position and lexing continues on the
same line. The token flows to the parser, which reports at the right line.

Two refinements, both small:

- **The lexer already has the right error variant and never constructs it.**
  `LexError::UnterminatedString { span }` exists at `lexer.rs:59` and no code
  produces it. Produce it when the unmatched character is `"` or `'`.
- **The parser's message should name the cause.** A generic "unexpected
  character" is not R6's "reported at its own line" in spirit. Special-case
  `Token::Error` carrying a quote.

**Note:** inside an IDENTIFICATION comment-entry no diagnostic should appear at
all — the entry is raw text and the collector simply walks over the error token.
That falls out for free and is the correct behaviour.

## 2. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-lexer/src/token.rs` | **the change** — two character classes, `[^"\\]` → `[^"\\\n]` and `[^'\\]` → `[^'\\\n]` |
| `crates/cobolt-lexer/src/lexer.rs` | construct `LexError::UnterminatedString` for an unmatched quote (currently dead) |
| `crates/cobolt-parser/src/…` | name the cause when a `Token::Error` holding a quote reaches a statement |
| `crates/cobolt-lexer/tests/test_literals.rs` | see §6 |
| `docs/cobol85-supported-syntax-en.md` | the ⚠️ note added at 1.62.11 telling developers to avoid an unpaired quote in a comment-entry becomes obsolete — replace it; re-measure the scoreboard |
| `specs/nist/*`, `version.rs`, `CHANGELOG.md` | as usual |

**Not touched:** `flatten_fixed_strict`, the COPY preprocessor, the EXEC RUST
capture.

## 3. Data / model changes

**None.** No token variant, no AST change, no format change.

## 4. Key decisions & alternatives

- **Decision: fix it in the regex, not by post-processing.** *Why:* it makes the
  bad state unrepresentable — no rule can produce a token spanning lines.
  *Rejected:* scanning tokens afterwards and splitting the ones that cross
  lines; same effect, more code, and it leaves the regex able to do the wrong
  thing.
- **Decision: do not add a `tokenize_with_errors` variant.** `tokenize()` drops
  `Lexer::errors` today, and it has **64 product call sites**. Changing its
  signature to thread errors through all of them is disproportionate for this
  fix. The diagnostic reaches the user through `Token::Error` reaching the
  parser, which is the path that already works. *Rejected:* a new API surface.
  Revisit if lexer diagnostics are wanted generally — that is its own spec.
- **Decision: keep `Token::Error` as the carrier.** It is already load-bearing:
  `data.rs:500` matches `Token::Error(s) if s == "$"` for the currency symbol in
  a PICTURE. Adding a second producer is consistent with how the lexer already
  reports the unrepresentable.
- **Decision: no free-format continuation.** The `&` token exists but is the
  **concatenation operator** (`expr.rs:517`), not COBOL-2002 line continuation.
  Free-format multi-line literals are therefore not a thing here, and this
  change cannot break them.

## 5. Risks & mitigations

| Risk | Mitigation |
|---|---|
| **A `EXEC RUST` block containing a multi-line Rust string breaks.** Rust *does* have multi-line strings, and Rust source is lexed by the same rules on its way past. | Measured-safe: the block is captured by **offset slicing** (`lexer.rs:390`) between the end of `RUST` and the start of `END-EXEC`, and the scan in between only looks for `Word("END-EXEC")` — error tokens are `Err(..)` and simply do not match. The captured text is byte-identical either way. **Residual risk:** a *multi-line* Rust string whose text contains the literal characters `END-EXEC` would today be swallowed and would now terminate the block early. Vanishingly unlikely; pin it with a test. |
| **A COBOL program legitimately relies on a literal crossing a line.** | None exists: fixed-format continuation is resolved by `flatten_fixed_strict` *before* lexing, and free format has no continuation. The one test that looks like a multi-line string (`test_literals.rs:237`, `StringLiteral("\r\n")`) is a **hex literal** `X"0D0A"`, unaffected. |
| **Error tokens change behaviour somewhere that matches `Token::Error` broadly.** | Only two matches exist, both for `"$"` in `data.rs`. Grep-verified; a test pins `PIC $9(5).99`. |
| **The 6 programs do not all clear**, because each may have a second blocker. | Expected — this is the lesson from 1.62.11 (`NIST-spec-identification-division-comment-entries.md` §10). Report the measured gain, not the bucket size. |
| **The scoreboard's ⚠️ note becomes wrong.** 1.62.11 added advice to avoid unpaired quotes in comment-entries; once fixed, that advice misleads. | Explicit doc task. |

## 6. Test strategy

**Lexer tests** (`cobolt-lexer`) — the containment property:

| Case | Asserts |
|---|---|
| `MOVE "abc TO X.` then a second line with `MOVE "def" TO Y.` | the stray quote does **not** consume line 2; line 2's literal is `def` |
| `THE COMPILER"S ABILITY` followed by 3 more lines and a balanced `"pair"` | the `"pair"` literal is intact — parity is restored at the newline |
| `MOVE "abc" TO X.` | unchanged (regression) |
| `X"0D0A"` | still `StringLiteral("\r\n")` — the hex rule is separate |
| `MOVE 'abc TO X.` | same containment for single quotes |
| A doubled `""` escape inside a one-line literal | unchanged |
| A fixed-format **continued** literal (`-` in column 7) under `FixedStrict` | still joins — the preprocessor runs first, so the joined text is one line by the time it is lexed. **This is the guard that matters most**: it proves 1.62.8's continuation work is untouched. |

**Parser test** — R6's reporting half: an unterminated literal in a statement
produces a diagnostic **on its own line**, and the statement after it still
parses.

**EXEC RUST test** (`cobolt-parser` or `cobolt-runtime`) — a block containing a
multi-line Rust string round-trips with identical captured source.

**Suite-level:**

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

Expect the 6-program `expected PROCEDURE DIVISION` bucket to clear and the SG
module to move off 0 / 13 — **but report the measured number.** Some of these
programs will meet segment priority numbers next
([`NIST-spec-segmentation.md`](NIST-spec-segmentation.md)), which is a different
spec, so the in-scope gain may be smaller than 6.

**Regression** — serially (concurrent `cargo test` deadlocks here):

```bash
cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic --no-fail-fast
cargo test -p cobolt-runtime --no-fail-fast
cargo test -p cobolt-forms --features render --no-fail-fast
```

The forms suite matters more than usual: generated `.cbl` files carry `EXEC RUST`
blocks, and this change touches how their interiors tokenize.

⚠️ **Keep the failure names.** Summarising a sweep with
`awk` over `^test result:` gives counts and discards *which* test failed. Use

```bash
cargo test -p cobolt-forms --features render --no-fail-fast 2>&1 | tee forms.log \
  | grep -E '^test result:|^test .* FAILED|^---- '
```

This is not hypothetical: the 1.62.11 sweep reported forms **616 passed / 1
FAILED**, and because the name had been discarded it could only be attributed by
inference. Four serial re-runs then gave 617 / 0, and the crate has exactly one
network-touching test (`tests/test_map_tiles_tls.rs`, a live fetch from
`tile.openstreetmap.org`), so that is almost certainly what flaked — but
"almost certainly" is what a preserved name would have made unnecessary.

## 7. Steering compliance

- [x] **i18n** — no new UI strings; lexer/parser diagnostics are compiler output.
- [x] **Generated-code contract** — `EXEC RUST` in generated form sources is the
      main exposure. §5's first risk row and the forms suite cover it.
- [x] **Docs** — English canonical only. Replace the 1.62.11 ⚠️ note and
      re-measure the scoreboard (`specs/steering/docs.md` requires the latter).
- [x] **System KB** — no control/property/method/event change.
- [x] **Fix vs feature** — **fix**; bump `z`, CHANGELOG, forum **f=97**.
- [x] **Branch** — `fixes`.

## 8. Open questions

- **Q1 — what should the diagnostic say?** Proposal: *"unterminated alphanumeric
  literal — a literal cannot span source lines; use a continuation line
  (`-` in column 7) in fixed format"*. It names the cause and the remedy, which
  matters because the remedy is non-obvious to someone writing free-format
  source. Confirm the wording.
- **Q2 — should an unpaired quote inside a comment-entry be reported at all?**
  Proposal: **no.** A comment-entry is raw text by definition, and the collector
  walking over the error token silently is correct. This falls out of the design
  and needs no code; flagged only so the behaviour is deliberate rather than
  accidental.
- **Q3 — the 2026-08-25 "follow NIST; where the standard has no reading, raise
  an error" ruling.** A literal spanning lines has no reading in COBOL-85, so an
  error is right. Confirming the ruling applies here as expected, not asking to
  revisit it.
