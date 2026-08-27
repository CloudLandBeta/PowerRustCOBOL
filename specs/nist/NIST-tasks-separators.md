# Tasks — separator comma, separator semicolon, space-separated subscripts

- **Spec:** [NIST-spec-separators.md](NIST-spec-separators.md)
- **Plan:** [NIST-plan-separators.md](NIST-plan-separators.md)
- **Status:** ✅ shipped
- **Measured:** 244 → 292 of 434 in-scope programs (**+48**)

## T1 — Enumerate every `Comma` / `Semicolon` consumer (spec Q2) ✅

Exhaustive `grep` over `crates/`, tabulated in the plan §1. Result:
`Token::Semicolon` has **zero** consumers; `Token::Comma` has thirteen, of which
only four are bounded *by* the comma and therefore need code changes.

Confirmed by reading, not assumed: `EXEC RUST … END-EXEC` is captured verbatim
by `try_capture_exec_rust` as a single `ExecRustBlock` token, so its Rust
semicolons are never COBOL tokens.

## T2 — Guard tests, failing against unmodified code ✅

`crates/cobolt-lexer/tests/test_literals.rs`: four separator tests
(`a_separator_comma_is_not_a_token`, `a_separator_semicolon_is_not_a_token`,
`a_separator_at_end_of_line_is_not_a_token`,
`separators_in_a_data_description_are_not_tokens`) — **all four failed** before
the change. Two companion tests (`a_glued_decimal_comma_survives`,
`a_glued_picture_comma_survives`) passed before *and* after: they are the
regression guards on what must NOT change.

## T3 — The lexer rule ✅

`crates/cobolt-lexer/src/lexer.rs`: `is_separator_punctuation()` plus a skip in
`next_token` placed **before** `classify()`, so a separator never becomes a
token and never touches `at_line_start`.

## T4 — Rebind the four comma-bounded loops ✅

| Site | Was | Now |
|---|---|---|
| `expr.rs` subscript list | `while p.eat(&Token::Comma)` | bounded by `RParen`, comma optional |
| `expr.rs` member subscript list | ” | ” |
| `expr.rs` member argument list | `if !p.eat(&Token::Comma) { break }` | bounded by `RParen` |
| `stmt.rs` `::method(…)` arguments | ” | bounded by `RParen` |

Each carries the liveness guard (`if p.pos == before { break }`) copied from the
`FUNCTION` argument loop that already read this way.

## T5 — R5: a subscript after the complete qualified name ✅

Surfaced by AC5 and **not** predicted by the plan. `CELL OF COLS OF ROWS
(IDX-A IDX-B)` was unparseable in *either* spelling, comma or space: the
subscript was only ever read **before** the `OF` chain, so the trailing list
fell through to the generic parenthesised-expression rule and reported
`expected RParen`. COBOL-85 specifies the opposite order —
`data-name-1 [OF data-name-2]… [(subscript…)]`.

Fixed by extracting `parse_subscript_or_refmod()` and applying it **both**
before and after the qualification loop, so both orders parse and nothing that
relied on the pre-qualification form changes. Worth +7 programs on its own
(285 → 292).

## T6 — Acceptance tests ✅

`crates/cobolt-parser/tests/test_separators.rs`, 11 tests covering AC1-AC6, AC8,
R7 and R8, plus a guard that the RustCOBOL member-call argument list did not
regress to a single argument.

AC4 and AC5 compare the **debug rendering of the whole parsed program** between
the spaced and comma spellings rather than asserting "it parsed" — an index
silently dropped from the list would satisfy the weaker assertion.

## T7 — One repo test updated, deliberately ✅

`test_literals.rs::period_comma_parens` asserted `t.contains(&Token::Comma)` for
`ADD A, B TO C (1).` — it pinned the pre-conformance behaviour this spec exists
to change. Renamed to `period_parens` and inverted, with the reason in a
comment. It is the repo's own unit test, not a user-provided one, so GOLDEN
RULE #2's report-don't-change clause does not apply; no other test changed.

## Coverage of the spec's acceptance criteria

| AC | Covered by | Status |
|---|---|---|
| AC1 `MOVE ZERO TO A, B, C.` | `ac1_a_comma_separated_receiver_list_parses` | ✅ |
| AC2 `PROCEDURE DIVISION USING A, B, C.` | `ac2_procedure_division_using_accepts_separator_commas` | ✅ |
| AC3 `CALL "SUB" USING A, B, C.` | `ac3_call_using_accepts_separator_commas` | ✅ |
| AC4 identical ASTs | `ac4_spaced_and_comma_separated_subscripts_are_identical`, `ac4_three_space_separated_subscripts_parse` | ✅ |
| AC5 qualified + spaced subscripts | `ac5_a_qualified_reference_takes_space_separated_subscripts` | ✅ |
| AC6 separators in a data description | `ac6_separators_inside_a_data_description_parse` | ✅ |
| AC7 named programs clear | measured — see below | ✅ |
| AC8 `DECIMAL-POINT IS COMMA` intact | `ac8_the_decimal_comma_still_works` + two lexer guards | ✅ |
| AC9 both census buckets empty | measured — see below | ✅ |

### AC7 / AC9 — measured, not asserted

Re-running the harness after the change:

| Bucket (before) | Progs | After |
|---|---:|---|
| `unexpected token in statement: Comma` | 50 | **0** |
| `expected RParen, found …` | 23 | **0** |
| `unexpected token in statement: Semicolon` | 14 | **0** |

Per-module movement, 244 → 292:

| Module | Before | After |
|---|---:|---:|
| NC | 30 | **56** |
| IC | 32 | **44** |
| IX | 31 | **38** |
| SQ | 47 | **50** |
| RL | 30 | **31** |
| DB | 9 | **10** |

## Done criteria

- [x] Every acceptance criterion covered by a test or a measurement
- [x] `cargo test -p cobolt-lexer` / `-p cobolt-parser` green
- [x] Full workspace sweep green (`--no-fail-fast`, every `test result:` read)
- [x] NIST re-measured and the scoreboard updated in
      `docs/cobol85-supported-syntax-en.md`
- [x] Version `z` bump + dated `CHANGELOG.md` entry
