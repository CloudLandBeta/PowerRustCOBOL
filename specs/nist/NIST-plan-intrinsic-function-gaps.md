# Plan — intrinsic function gaps: the `(ALL)` table argument

- **Spec:** [NIST-spec-intrinsic-function-gaps.md](NIST-spec-intrinsic-function-gaps.md)
- **Status:** R1-R4 shipped; R5/R6 open
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-26
- **Classification:** fix. Forum f=97.

## 1. Approach

### R1-R3 — `ALL` as a subscript

`FUNCTION MAX(IND(ALL))` hands a whole table to a statistical intrinsic. One
*written* argument becomes as many *actual* arguments as the table has
occurrences, so the expansion has to happen where arguments are turned into
values — `Interpreter::eval_args` — and nowhere else. Every variable-length
intrinsic (`MAX`, `MIN`, `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`,
`VARIANCE`, `STANDARD-DEVIATION`, `ORD-MAX`, `ORD-MIN`) already consumes
whatever `eval_args` returns, so all eleven are covered by one change.

Three layers, each doing only its own job:

| Layer | Change |
|---|---|
| `cobolt-ast` | new `Expr::AllSubscript(Span)` — a marker, carrying no value |
| `cobolt-parser` | `parse_subscript_index()` produces it in a subscript position |
| `cobolt-runtime` | `eval_args` expands it; `expand_all_subscript` enumerates |

### The disambiguation — position, not lookahead

`ALL` already means the figurative-constant prefix (`ALL "X"`), and R4 requires
that to be untouched. Spec Q2 recommends the parser over the lexer for exactly
this reason, and that recommendation is followed.

**The rule is positional: inside a subscript list, `ALL` is always the
table-wide meaning.** A subscript is an integer expression or an index-name; a
figurative constant is never a legal subscript, so there is no ambiguity to
resolve.

A lookahead rule — "`ALL` followed by a literal is figurative" — was written
first and is **wrong**, which the AC2 test caught. Once separator commas stop
being tokens (`NIST-spec-separators.md`), `TBL(ALL, 2)` reaches the parser as
`TBL(ALL 2)`, and lookahead reads that as the figurative constant `ALL 2`. The
2-D case silently summed nothing. Position is the only reliable signal.

### Row-major order

`ALL` may appear in any dimension with ordinary subscripts in the others
(R2). `expand_all_subscript` enumerates the `ALL` dimensions as an odometer
with the **last** varying fastest — the order the table is laid out in, and
therefore the order `ORD-MAX` / `ORD-MIN` must report an ordinal against.

R3 (`OCCURS … DEPENDING ON`) needs no special handling: `dims_of` reads the
environment's current occurrence data, so the expansion is against the count at
call time by construction.

### Refusing to guess

If the base has no OCCURS metadata, or any dimension is zero, the reference is
evaluated as written instead of expanded. Inventing a count would hand the
function the wrong number of arguments and produce a confidently wrong number —
worse than the diagnostic it replaced.

## 2. Two defects found by the acceptance tests

Neither was predicted by the spec; both are pre-existing and both are fixed.

**`MOVE ALL "X" TO X` filled one character.** `FigurativeConstant::All(inner)`
evaluated to `literal_to_value(inner)` — the `ALL` was dropped entirely, so the
literal landed once and the rest of the field stayed spaces. `ALL` is the only
figurative constant whose fill character is more than one byte, which is why
SPACES and ZEROS never showed the bug. Fixed in `exec_move`, the only place the
receiver's declared width is known. The repo's own `tests/cobol/fileio/*.cbl`
use `MOVE ALL "X" TO PERF-PAYLOAD` to build a payload and were getting one `X`.

**`MOVE ALL ZEROS` was rejected** with "expected literal after ALL". COBOL-85
allows `ALL` before another figurative constant, where it is redundant —
`ALL ZEROS` is `ZEROS`. Only a real literal was accepted.

## 3. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-ast/src/expr.rs` | `Expr::AllSubscript` + its `span()` arm |
| `crates/cobolt-parser/src/expr.rs` | `parse_subscript_index`; `ALL` before a figurative constant |
| `crates/cobolt-semantic/src/resolver.rs` | walk arm (nothing to resolve) |
| `crates/cobolt-runtime/src/interpreter.rs` | `eval_args`, `expand_all_subscript`, `exec_move` fill, `eval_expr` arm |
| `crates/cobolt-runtime/src/environment.rs` | `alphanumeric_capacity()` |
| `crates/cobolt-runtime/tests/test_all_subscript.rs` | 5 tests |

## 4. Key decisions & alternatives

- **Positional, not lookahead** — see above; the alternative was measured wrong.
- **Expand in `eval_args`, not in the parser.** The parser does not know how
  many occurrences a table has, and the count can change at run time under
  `DEPENDING ON`.
- **`AllSubscript` errors if evaluated as a value.** It has no value; reaching
  `eval_expr` means it was written where the expansion does not apply, and
  saying so beats inventing a number.

## 5. Open — R5 and R6, not shipped

**R5 (unknown `FUNCTION` name becomes a compile error) is NOT implemented.**
Today an unrecognised name logs a warning and returns 0. Spec Q1 flags this as a
behaviour change and asks for an operator ruling on **error vs warning**; the
recommendation is error, naming the closest implemented function.

The blocker is structural, and worth recording: the implemented set lives in
`eval_function`'s `match` in `cobolt-runtime`, and the check belongs in
`cobolt-semantic`, which does not depend on `cobolt-runtime` (and cannot —
`cobolt-stdlib` already depends on `cobolt-runtime`). Both depend on
`cobolt-ast`, so the list belongs there, with a test in `cobolt-runtime`
asserting every listed name is handled — otherwise the list and the `match`
drift apart silently, which is the same class of bug as the one this spec fixed.

**R6's audit is done** and is recorded here rather than left implicit. The 53
names `eval_function` implements:

`ABS ACOS ANNUITY ASIN ATAN BYTE-LENGTH CHAR CONCATENATE COS CURRENT-DATE
DATE-OF-INTEGER DAY-OF-INTEGER EXP EXP10 FACTORIAL FRACTION-PART INTEGER
INTEGER-OF-DATE INTEGER-OF-DAY INTEGER-PART LENGTH LENGTH-AN LOG LOG10
LOWER-CASE MAX MEAN MEDIAN MIDRANGE MIN MOD NUMVAL NUMVAL-C NUMVAL-F ORD
ORD-MAX ORD-MIN PI PRESENT-VALUE RANDOM RANGE REM REVERSE SIN SQRT
STANDARD-DEVIATION STORED-CHAR-LENGTH SUM TAN TEST-NUMVAL TRIM UPPER-CASE
WHEN-COMPILED YEAR-TO-YYYY`

**The IF module is now 45 / 45**, so no function the module calls is missing.
That is the audit R6 asks for, measured rather than asserted.

## 6. Test strategy

`crates/cobolt-runtime/tests/test_all_subscript.rs` — end-to-end, running the
program and reading its DISPLAY output, because the point of the change is the
*values* the function receives, not that it parsed. AC2 asserts both a single
column (12) and every cell (21) so a wrong enumeration order or a dropped
element cannot pass.

## 7. Measured

| | IF module | in-scope PASS |
|---|---:|---:|
| before | 29 / 45 | 292 |
| after | **45 / 45** | **303** |
