<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 Supported Syntax Reference

**Ground truth of what the RustCOBOL lexer/parser/runtime actually accept today**,
derived from the source (`cobolt-lexer`, `cobolt-parser`, `cobolt-runtime`).
Write tests against the ✅ forms; the ❌ forms will fail to parse or are no‑ops,
and ⚠️ forms parse but behave partially. This is the companion to
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md): the matrix says
*what* to test, this says *which spelling RustCOBOL understands*.

Legend: ✅ supported · ⚠️ parses but partial/simplified · ❌ not recognized
(avoid, or test only to confirm the gap).

> **Update (gap-implementation pass):** the following were implemented and are
> now ✅ — **reference modification** `id(start:len)`, **inline
> `PERFORM n TIMES`**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING `ON OVERFLOW` +
> `END-STRING`/`END-UNSTRING`**, **category-aware `INITIALIZE`**, **operator-
> prefixed abbreviated conditions** (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`**
> (runs on unresolved CALL), **`COMPUTE` multiple receivers + per-receiver
> `ROUNDED`**, a much larger **intrinsic-function set**, and extended
> `ACCEPT`/`DISPLAY` screen forms (parsed, not executed).
>
> **Update (hierarchical / occurrence-aware environment pass — 1.5.0):** four
> data-model-blocked features are now ✅ — **runtime table subscripting** `t(i)`
> / `t(i, j)` (per-occurrence storage), **qualified-name disambiguation**
> `id OF/IN group` (duplicated leaf names resolve to independent storage),
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**, and **functional `SEARCH` / `SEARCH ALL`**.
> The avoid-list at the bottom is current.

---

## Recognized statements (verbs)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `STOP` `EXIT` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET`
⚠️ `SORT` `MERGE` (parsed; runtime incomplete) · `INITIALIZE` (→ `MOVE SPACES`) ·
`INVOKE` (parsed as no‑op) · `CANCEL` (skipped)
Project extensions: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`, `THROW`.

✅ `SEARCH` / `SEARCH ALL` (functional — drives the table index and runs the
first matching `WHEN`, else `AT END`).
⚠️ **Recognized but no‑op** (parse cleanly, do nothing yet): `RELEASE`,
`RETURN`, `UNLOCK`, `ALTER`.
❌ **Not recognized — do not use:** `ENTRY`, `USE`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Per‑verb supported forms

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (multiple receivers).
- ✅ `MOVE CORRESPONDING g1 TO g2` — moves each subordinate item the two groups
  share by name, recursing through matching sub-groups.
- ✅ **Reference modification `id(start:len)`** — sender (substring) and receiver
  (spliced partial assignment); works on every verb's operands. `length` optional.
- ✅ subscripts `t(i)`, `t(i, j)` — read/write the per-occurrence storage slot;
  variable subscripts `t(WS-I)` evaluated each access.
- ✅ qualification `id OF/IN group` (`… OF g1 OF g2`) — resolves to the correct
  item even when the leaf name is declared under more than one group.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [r2 …] [ROUNDED] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r [ROUNDED] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ⚠️ `ROUNDED` is **one flag for the whole statement**, not per‑receiver.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine each matching numeric
  pair, recursing through matching sub-groups.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [GIVING r] [ROUNDED] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [GIVING q] [REMAINDER r] [ROUNDED] [SIZE ERROR …][END-DIVIDE]`.
- ❌ multiple receivers (`MULTIPLY a BY r1 r2`), `DIVIDE … GIVING q1 q2`.

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **multiple receivers, each with its own `ROUNDED`**.
- ✅ expr operators `+ - * /` and `**` (power, right‑assoc), parentheses,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE}` … `WHEN {value | value THRU value | ANY |
  condition | OTHER} [ALSO …] stmts … [WHEN OTHER stmts] END-EVALUATE`.
- ⚠️ `ALSO` (multi‑subject) is collected but evaluation is simplified; `NOT value`
  in a WHEN is stored as the plain value (negation simplified).

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = integer literal or data‑item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ inline `PERFORM UNTIL cond … END-PERFORM`,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ inline `PERFORM n TIMES … END-PERFORM` (no paragraph).
- ⚠️ `PERFORM p VARYING …` ignores the paragraph name.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ⚠️ `EXIT` and `EXIT PROGRAM` both compile to `STOP RUN` (return to caller).
- ❌ `EXIT PERFORM [CYCLE]`, `EXIT PARAGRAPH`, `EXIT SECTION`, `NEXT SENTENCE`.

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ⚠️ **screen forms parse but do not execute** (SCREEN I/O superseded by the form
  designer): `ACCEPT id AT nnnn`, `… AT LINE n COLUMN n`, `… WITH <attributes>`.
- ⚠️ `FROM {ENVIRONMENT-VALUE | ARGUMENT-NUMBER | ARGUMENT-VALUE | ESCAPE KEY |
  CRT STATUS}` are recognized as no‑op sources.

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING]`.
- ⚠️ screen forms `DISPLAY id AT nnnn`, `AT LINE n COLUMN n`, `WITH <attributes>`
  parse but are ignored (designer supersedes SCREEN I/O).

### STRING
- ✅ `STRING {src DELIMITED BY {SIZE | delim}} … INTO target [WITH POINTER p]
  [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`. Overflow = the
  assembled string is wider than the receiving field.

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Overflow = more source fields than
  receivers.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x} …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y …`.
- ⚠️ `INSPECT … TALLYING … REPLACING …` — the **REPLACING half is skipped**.
- ❌ `BEFORE/AFTER INITIAL` phrases (not parsed in TALLYING/REPLACING).

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compiled to MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encoded as ADD / SUBTRACT).
- ⚠️ `SET 88-name TO TRUE` — encoded as MOVE 1; verify it sets the host correctly.
- ❌ `SET ADDRESS OF …`, `SET pointer TO {ADDRESS OF … | NULL}`.

### INITIALIZE
- ✅ `INITIALIZE id …` — category-aware: numeric / numeric-edited → ZERO,
  everything else → SPACES, recursing into group items.
- ⚠️ `REPLACING …` is parsed but skipped.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp [NOT …]] [END-CALL]`.
- ✅ The `ON EXCEPTION` / `ON OVERFLOW` body **runs** when the called program is
  unresolved. (`NOT ON EXCEPTION` is parsed but its body not yet run.)
- ⚠️ `CANCEL p` is skipped (no‑op).

### File verbs (the supported phrases — full coverage is in the file‑I/O suite)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f …`; `CLOSE f …`.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ❌ `OPEN … SHARING`, `… WITH LOCK`, `READ … WITH [NO] LOCK`, `UNLOCK`.

### SORT / MERGE  ⚠️ (parsed, runtime incomplete — confirm before relying on)
- `SORT f {ASCENDING|DESCENDING} KEY k … [USING …|INPUT PROCEDURE p [THRU p2]]
  [GIVING …|OUTPUT PROCEDURE p [THRU p2]] [END-SORT]`.
- `MERGE f {ASCENDING|DESCENDING} KEY k … [OUTPUT PROCEDURE p] [END-MERGE]`.

---

## Conditions (IF / EVALUATE / PERFORM UNTIL)

- ✅ Relational symbols: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Word relations: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Class: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
- ✅ Sign: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ 88‑level condition‑name (bare name as a condition).
- ✅ Combined `AND` / `OR` / `NOT`, parentheses (AND binds tighter than OR).
- ✅ **Operator‑prefixed abbreviated conditions** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (the preceding comparison subject is reused).
- ❌ **Identifier‑object abbreviation** (`a = b OR c`, where `c` is a data‑item)
  — needs semantic resolution; use the operator‑prefixed form or repeat the LHS.

---

## Expressions, literals, USAGE

- ✅ Arithmetic operators `+ - * /` and `**`; parentheses; unary `+`/`-`.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — **implemented** intrinsics:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED`.
  ⚠️ Any **other** `FUNCTION` name parses but returns **0** at runtime (e.g.
  `DATE-OF-INTEGER, INTEGER-OF-DATE, ANNUITY, …`).
- ✅ Literals: integer, decimal, string, all figurative constants
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).

---

## DATA DIVISION clauses (declaration syntax accepted)

- ✅ Levels `01`–`49`, `77`, `88`; `FILLER`; group/elementary.
- ✅ `PIC/PICTURE` with `X A 9 S V P` and edited symbols (`Z * $ + - CR DB B 0 /
  , .`).
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (and `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numeric/signed/alphanumeric/figurative/`ALL`).
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES`, `JUSTIFIED [RIGHT]`, `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL`.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b`.
- ⚠️ `USAGE INDEX` / `USAGE POINTER` — tokens exist; confirm they declare/behave.
- ⚠️ `66 RENAMES` — `RENAMES` is a token but 66‑level handling is unverified.
- Sections: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN`
  parsed but not executed.

---

## Still NOT supported — current avoid‑list

These remain ❌ (parse error) or ⚠️ (parsed/recognized but a no‑op):

1. **Multiple receivers on `MULTIPLY`/`DIVIDE`** — and `ADD`/`SUBTRACT`
   per‑receiver `ROUNDED` (still statement level). (`COMPUTE` multiple receivers
   + per‑receiver `ROUNDED` **is** done; `ADD/SUBTRACT CORRESPONDING` **is** done.)
2. **`SET ADDRESS OF`**, `SET pointer TO {ADDRESS OF … | NULL}`.
3. **Identifier-object abbreviated conditions** (`a = b OR c` where `c` is a
   data‑item) — needs semantic resolution; the **operator‑prefixed** form
   (`a > 1 AND < 9`, `a = 5 OR = 7`) **is** supported.
4. **Screen `ACCEPT`/`DISPLAY` execution** — `AT`/`WITH` phrases parse and are
   ignored (the form designer supersedes SCREEN SECTION I/O).
5. **`RELEASE`/`RETURN`/`UNLOCK`/`ALTER`** — recognized (parse) but no‑ops
   (`RELEASE`/`RETURN` await the full `SORT` runtime).
6. **`INSPECT … TALLYING … REPLACING`** combined (REPLACING half skipped) and
   `INSPECT … BEFORE/AFTER INITIAL`.
7. Intrinsics outside the implemented set still return **0**.

> A test that *intentionally* targets one of these (to drive the fix) will hit a
> parser diagnostic or a no‑op — that is the signal for the next pass.
>
> **Resolved (1.5.0):** the **flat data model** has been replaced by a
> hierarchical / occurrence‑aware environment, unblocking **CORRESPONDING**
> (`MOVE`/`ADD`/`SUBTRACT`), **qualified names**, **table subscripting**, and
> **functional `SEARCH` / `SEARCH ALL`** together.
