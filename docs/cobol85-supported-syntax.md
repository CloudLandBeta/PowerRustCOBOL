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
> `ROUNDED`**, and a much larger **intrinsic-function set**.
>
> **Update (hierarchical / occurrence-aware environment pass — 1.5.0):** four
> data-model-blocked features are now ✅ — **runtime table subscripting** `t(i)`
> / `t(i, j)` (per-occurrence storage), **qualified-name disambiguation**
> `id OF/IN group` (duplicated leaf names resolve to independent storage),
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**, and **functional `SEARCH` / `SEARCH ALL`**.
>
> **Update (verb-completeness pass — 1.6.0):** now also ✅ — **multi-receiver
> `MULTIPLY`/`DIVIDE GIVING` + per-receiver `ROUNDED`** on `ADD`/`SUBTRACT`;
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** and corrected
> plain `EXIT`; **`CALL … NOT ON EXCEPTION`**; **`INSPECT … TALLYING …
> REPLACING`** combined and **`BEFORE/AFTER INITIAL`** regions; date/financial
> **intrinsics** (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`,
> `DAY-OF-INTEGER`, `ANNUITY`, `FRACTION-PART`); **literal-object abbreviated
> conditions** (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-subject) and
> **`WHEN NOT`**; **real 88-level condition-names** (`SET … TO TRUE/FALSE`, host
> tested against its VALUEs/ranges); **`PERFORM para VARYING`**; and a functional
> **`SORT`/`MERGE`** runtime (`RELEASE`/`RETURN`, `USING`/`GIVING`, `INPUT`/`OUTPUT
> PROCEDURE`). The avoid-list at the bottom is current.
>
> **Update (avoid-list clearance pass — 1.7.0):** the remaining gaps are now
> implemented — **identifier-object abbreviation** (`a = b OR c`, resolved via
> 88-level metadata); **`INITIALIZE … REPLACING category DATA BY value`**;
> **`66 RENAMES`** (read synthesizes / write distributes across covered items);
> **pointers** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> `SET ADDRESS OF item TO …` aliasing, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; faithful **`NEXT SENTENCE`**; the remaining standard
> **intrinsics** (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`,
> `TEST-NUMVAL`); and extended **screen `ACCEPT`/`DISPLAY`** (`AT`/`WITH` via
> ANSI in CLI mode — now *executed*, not just parsed).
>
> **Update (1.7.1):** the `ACCEPT` register sources are now functional (were
> recognized no-ops) — **`FROM COMMAND-LINE`**, **`ARGUMENT-NUMBER`** /
> **`ARGUMENT-VALUE`** (paired with `DISPLAY n UPON ARGUMENT-NUMBER`),
> **`ENVIRONMENT-VALUE`** (paired with `DISPLAY "name" UPON ENVIRONMENT-NAME`),
> **`ESCAPE KEY`** → `"00"`, **`CRT STATUS`** → `"0000"`.
>
> **Update (1.7.2):** file-sharing / locking phrases and `CANCEL` (were ❌ /
> no-op) — **`OPEN … SHARING WITH … [WITH LOCK]`**, **`READ … WITH [NO] LOCK`**,
> **`UNLOCK`** (releases the file's INDEXED record locks), and **`CANCEL program`**
> (re-initialises the program's storage). The avoid-list at the bottom is current.

---

## Recognized statements (verbs)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirects para-1's `GO TO`) ·
`UNLOCK file` (releases the file's record locks) · `OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (file sharing/locking — advisory in the single run unit)
✅ `CANCEL` (re‑initialises the program's storage) · ⚠️ `INVOKE` (parsed as no‑op)
Project extensions: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`, `THROW`.

✅ `SEARCH` / `SEARCH ALL` (functional — drives the table index and runs the
first matching `WHEN`, else `AT END`). ✅ `SORT` / `MERGE` with `RELEASE` /
`RETURN` (functional — see below).
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
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **per‑receiver `ROUNDED`** — each receiver carries its own `ROUNDED` flag.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine each matching numeric
  pair, recursing through matching sub-groups.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **multiple `GIVING` receivers**, each with its own `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (no `GIVING`) stores `a/b` back into `a` (a PowerRustCOBOL
  convenience; standard COBOL requires `INTO` or `GIVING` here).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **multiple receivers, each with its own `ROUNDED`**.
- ✅ expr operators `+ - * /` and `**` (power, right‑assoc), parentheses,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **`ALSO` multi‑subject** — each `WHEN` column is matched positionally
  against its subject and AND‑combined.
- ✅ **`WHEN NOT value`** negates a selection object; **`WHEN condition`**
  (e.g. `EVALUATE TRUE WHEN a > b`) evaluates the boolean condition.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = integer literal or data‑item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ inline `PERFORM UNTIL cond … END-PERFORM`,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ inline `PERFORM n TIMES … END-PERFORM` (no paragraph).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — runs the paragraph each
  iteration (out‑of‑line, no `END-PERFORM`).

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ plain `EXIT` is a no‑op return point; `EXIT PROGRAM` returns to the caller.
- ✅ `EXIT PERFORM [CYCLE]` (break / continue the nearest inline PERFORM),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfers control past the next sentence boundary (the
  parser inserts boundary markers at each period; faithful, not just `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` positions the cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (whole command line) · `FROM ARGUMENT-NUMBER` (arg count)
  · `FROM ARGUMENT-VALUE` (arg at the pointer set by `DISPLAY n UPON
  ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE` (the
  variable named by `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`.

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING]`.
- ✅ screen forms `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — executed via ANSI cursor
  positioning + SGR in **CLI mode** (`rcrun`); ignored in GUI mode (the form
  designer supersedes SCREEN I/O there). `ACCEPT id AT …` positions then reads.

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
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **both halves applied**.
- ✅ `BEFORE/AFTER INITIAL` confines each phrase to a sub‑region of the field.
  (TALLYING accumulates onto the counter, per COBOL.)

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compiled to MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encoded as ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` sets the host item to the condition's first VALUE;
  `TO FALSE` sets a value outside the VALUE set (best effort — no FALSE clause).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` and
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — see **Pointers** below.

### INITIALIZE
- ✅ `INITIALIZE id …` — category-aware: numeric / numeric-edited → ZERO,
  everything else → SPACES, recursing into group items.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — sets each
  subordinate item of that category to the value; others untouched.

### Pointers (USAGE POINTER)
- ✅ `USAGE POINTER` declares a pointer (NULL initially).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — aliases `id` onto the
  target's storage (reads **and** writes follow the alias); typically a LINKAGE
  record. `IF ptr = NULL` works.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ The `ON EXCEPTION` / `ON OVERFLOW` body runs when the called program is
  unresolved; the `NOT ON EXCEPTION` body runs when the call **resolves**.
- ✅ `CANCEL program …` re-initialises the named program's WORKING-STORAGE so its
  next `CALL` starts fresh.

### File verbs (the supported phrases — full coverage is in the file‑I/O suite)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK]`; `CLOSE f …`. (`SHARING` / `WITH LOCK` parse and are honoured
  where meaningful — advisory in the single‑run‑unit model.)
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` releases the record lock the INDEXED engine takes under I‑O.
- ✅ `UNLOCK f [RECORD[S]]` releases the file's record locks.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ Cross‑*process* file sharing is not enforced (single run unit); the
  `SHARING`/`LOCK` phrases parse and the INDEXED engine's per‑run record locks
  are honoured.

### SORT / MERGE / RELEASE / RETURN  ✅ (functional, in‑memory work buffer)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (in an INPUT PROCEDURE) appends to the run;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` hands records back.
- Records are stable‑sorted by the declared keys (`ASCENDING`/`DESCENDING`);
  `USING` reads / `GIVING` writes the named sequential files.

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
- ✅ **Literal‑object abbreviation** — `a = 1 OR 2 OR 3` (reuses both the subject
  and the operator; the object is a literal).
- ✅ **Identifier‑object abbreviation** — `a = b OR c` (where `c` is a data‑item).
  A bare identifier after AND/OR following a comparison is resolved at runtime:
  a known 88‑level condition‑name evaluates as one, otherwise it is the object
  `a = c`. (An identifier immediately followed by `AND` keeps AND precedence.)

---

## Expressions, literals, USAGE

- ✅ Arithmetic operators `+ - * /` and `**`; parentheses; unary `+`/`-`.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — **implemented** intrinsics:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Date conversions use the standard base 1601‑01‑01 = day 1.) The **complete
  COBOL‑85 standard intrinsic set** is implemented.
  ⚠️ Any unrecognised `FUNCTION` name still parses but returns **0** at runtime.
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
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **real condition‑names**: the
  level‑88 binds to its host item; testing checks the host against the VALUEs /
  ranges, and `SET 88-name TO TRUE` stores a satisfying value into the host.
- ✅ `USAGE INDEX` declares an integer index register (`SET`/`SEARCH` use it);
  `USAGE POINTER` — see **Pointers** above.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — a regrouping alias;
  reading concatenates the covered items, writing distributes by field width.
- Sections: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN`
  parsed but not executed.

---

## Still NOT supported — current avoid‑list

The COBOL‑85 verb / clause set is **fully covered**. What remains outside scope
is intentional or post‑85:

1. **Screen `ACCEPT` input editing** — `DISPLAY … AT/WITH` and `ACCEPT … AT`
   are executed (ANSI) in CLI mode, but full field‑level SCREEN SECTION editing
   (auto‑tab, field validation, colour maps) is **superseded by the form
   designer** in GUI mode.
2. **Cross‑*process* file sharing** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK`, and `UNLOCK` parse and drive the INDEXED engine's
   per‑run record locks, but locks are not enforced across separate OS processes
   (single run‑unit model).
3. **Object‑Oriented COBOL** (class/method definitions) — `INVOKE` is a no‑op
   for COBOL objects (it drives GUI/runtime objects only).
4. **RELATIVE** file organization (SEQUENTIAL / LINE SEQUENTIAL / INDEXED done).
5. Unrecognised intrinsic‑function names still return **0**.

> **Resolved (1.5.0):** the flat data model became hierarchical / occurrence‑aware,
> unblocking **CORRESPONDING**, **qualified names**, **table subscripting**, and
> **`SEARCH`**.
> **Resolved (1.6.0):** multi‑receiver `MULTIPLY`/`DIVIDE` + per‑receiver
> `ROUNDED`; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`; combined
> `INSPECT TALLYING REPLACING` + `BEFORE/AFTER INITIAL`; date/`ANNUITY`
> intrinsics; literal‑object abbreviation; `EVALUATE ALSO`/`WHEN NOT`; real
> 88‑level condition‑names; `PERFORM para VARYING`; and the `SORT`/`MERGE`
> runtime with `RELEASE`/`RETURN`.
> **Resolved (1.7.0):** identifier‑object abbreviation; `INITIALIZE … REPLACING`;
> `66 RENAMES`; pointers (`USAGE POINTER`, `SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`); `ALTER` / `UNLOCK`; faithful `NEXT SENTENCE`; the remaining standard
> intrinsics; and extended screen `ACCEPT`/`DISPLAY` (executed in CLI mode).
> **Resolved (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (with the paired
> `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` registers).
> **Resolved (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (releases INDEXED record locks), and `CANCEL program`.
