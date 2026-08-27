# Tasks — statement-level grammar gaps

- **Spec:** [NIST-spec-statement-grammar-gaps.md](NIST-spec-statement-grammar-gaps.md)
- **Status:** partially shipped — gaps 2, 4, 5, 6 done; 1, 3, 7 open
- **Measured:** contributed to 303 → 317 of 434 in-scope

The spec groups seven independent gaps. Each is tracked separately here, with
the ones **not** done named as plainly as the ones that are.

## ✅ Gap 6 (R12/R13) — qualified + subscripted operand

Delivered by `NIST-spec-separators.md`, not by this spec: extracting
`parse_subscript_or_refmod()` and applying it after the `OF`/`IN` chain made
`SET INDEX1 TO TABLE2-REC OF TABLE2 (INDEX2)` and
`CELL OF COLS OF ROWS (IDX-A IDX-B)` parse in every statement at once, because
they all go through the same primary-expression parser. R13 (space-separated
subscripts in that position) came with it.

## ✅ Gap 5 (R10/R11) — CLOSE reel/unit and lock phrases

`crates/cobolt-parser/src/stmt.rs` `parse_close`:
`CLOSE file [{REEL|UNIT} [FOR REMOVAL]] [WITH {NO REWIND|LOCK}]`.

None of `REEL`, `UNIT`, `FOR`, `REMOVAL`, `LOCK`, `REWIND` is a lexer keyword,
so they are matched by spelling with the existing `is_word()` — the same way
`OPEN … WITH REGISTERED USER` is parsed, and with no new reserved words, which
would otherwise stop a developer naming an item `LOCK`.

Matching them also stops the old loop reading `LOCK` as *the next file in the
list*, which is what it did.

**R11 is implemented, not stubbed.** `Stmt::Close` gained `locked: Vec<String>`;
`Interpreter` gained `locked_files`, and `exec_open` reports **file status 38**
for a file closed `WITH LOCK`. Reel/unit are multi-volume tape positioning and
are accepted as no-ops on disk — stated in the code, consistent with the
existing single-run-unit model.

## ✅ Gap 4 (R8/R9) — signed literal and THRU in WHEN

`parse_signed_literal()` folds a leading `+`/`-` into the literal. The lexer
deliberately emits the sign separately (`COMPUTE X = Y - 3.14` needs the
operator), so every site wanting a *signed literal* must fold it, and `WHEN` did
not: `WHEN -0.000020 THRU 0.000020` reported `expected comparison operator in
condition`.

It **rewinds to the original position** when what follows the sign is not a
numeric literal, so a condition-shaped `WHEN` still reaches `parse_condition`.

## ✅ Gap 2 (R3/R4) — PERFORM … TIMES with an identifier

`PERFORM PFM-C THREE TIMES.` An identifier is accepted as the count **only when
`TIMES` actually follows it**, so a plain `PERFORM PARA-A` followed by an
unrelated statement is untouched.

R4 (evaluate the count once) needed no change — `PerformTarget::Times` already
evaluates it before the loop. Verified by reading, not assumed.

## ✅ Not in the spec — a count that spills onto the next line

Found while measuring, and worth its own entry because the cause is not
statement grammar at all:

```cobol
    10  STUFF-1 OCCURS
            31 TIMES.
```

The lexer decides between `IntegerLiteral` and `LevelNumber` from **position** —
a number that opens a line is taken for a level number — and it cannot do
better, since a level number is only recognisable from context. So a clause
whose count spills onto the next line arrives mis-typed, and `OCCURS` reported
`expected integer after OCCURS`. This is the whole `LevelNumber(n)` family of
diagnostics in the census.

Fixed where an integer is **syntactically required** (`eat_required_integer` for
`OCCURS` / `OCCURS … TO`, and the `PERFORM … TIMES` count), where a real level
number can never appear — so accepting both spellings is exact, not lenient.
Deliberately *not* fixed by making `LevelNumber` an integer everywhere: the data
parser relies on that token to start an entry.

## ⬜ Gap 1 (R1/R2) — MULTIPLY / DIVIDE with multiple receivers — NOT DONE

`MULTIPLY 2 BY A ROUNDED B ROUNDED C.` still fails. R2 (each receiver computed
from the *original* operand value, not from an already-updated one) is the part
that needs care.

## ⬜ Gap 3 (R5-R7) — EVALUATE with a class-condition subject — NOT DONE

`EVALUATE WRK-XN-00001-1 NUMERIC … WHEN TRUE` still fails, with a cascade that
swallows the `WHEN` branches and `END-EVALUATE`.

## ⬜ Gap 7 (R14) — INSPECT … CONVERTING — NOT DONE

## Coverage of the spec's acceptance criteria

| AC | Status |
|---|---|
| AC1 / AC2 — MULTIPLY multiple receivers | ⬜ not done (gap 1) |
| AC3 — `PERFORM PFM-C THREE TIMES` | ✅ |
| AC4 — `EVALUATE X NUMERIC` | ⬜ not done (gap 3) |
| AC5 — `WHEN -0.000020 THRU 0.000020` | ✅ parses; **range matching not yet asserted by a test** |
| AC6 — `CLOSE F WITH LOCK` then reopen | ✅ parses; status 38 implemented, **not yet asserted by a test** |
| AC7 — `SET INDEX1 TO TABLE2-REC OF TABLE2 (INDEX2)` | ✅ (via separators) |
| AC8 — `INSPECT … CONVERTING` | ⬜ not done (gap 7) |
| AC9 — named programs lose these diagnostics | ✅ measured: IF module 45/45, IX 38→40, ST 30→32, SQ 50→52 |
| AC10 — supported-syntax doc corrected | ✅ |

**AC5 and AC6 are honestly incomplete.** Both constructs parse and both are
implemented, and the module movement is measured — but neither has a test
asserting the *behaviour* (that the range matches the right values; that
reopening a locked file returns 38). They are listed as ✅-parse / ⬜-behaviour
rather than ticked, because a parse-level pass is exactly what this project's
own rule says not to count as done.

## Deliberately NOT done — `WRITE … AT END-OF-PAGE`

Four programs (SQ201M, SQ208M, SQ209M, SQ401M) fail on
`WRITE PRINT-REC BEFORE ADVANCING 1 LINE AT END-OF-PAGE`. Parsing the phrase is
a ten-line change and would move all four to PASS.

**It was not done on purpose.** There is no `LINAGE` support anywhere in the
parser or runtime, and `AT END-OF-PAGE` only has meaning against a LINAGE page.
Parsing it without LINAGE would convert an honest compile error into a program
that runs and takes the wrong branch in silence — which is what
`specs/nist/README.md` calls the worst possible failure, and it would inflate
the PASS figure without making anything work.

It belongs to `NIST-spec-linage-and-io-control.md`, with LINAGE.
