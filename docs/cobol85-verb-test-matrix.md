<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 Verb & Data‑Section Test Matrix

A test-specification for finishing COBOL‑85 within the project scope. It
enumerates, **in depth**, what is *not yet covered* by the existing suites, as
syntax skeletons + permutation axes + the data-type mixing each verb must be
driven with. The goal of these tests is **exploratory**: run every variation,
observe current behavior, and decide what to fix / adjust / create / remove.

> Already verified — DO NOT re-spec here: exact numeric arithmetic
> (ADD/SUB/MUL/DIV/COMPUTE result values, ROUNDED, ON SIZE ERROR), numeric‑edited
> PICTUREs + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, all file I/O
> (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, keys, START/REWRITE/DELETE/INVALID KEY,
> STORAGE MODE, compression), nested programs/basic CALL, alphanumeric compare,
> lexer fixed/free. (Arithmetic *syntax* permutations below are still in scope —
> only the value math is "done".)

## Notation

- `[ x ]` optional, `{ a | b }` choice, `…` repetition, `dn` = data-item n.
- **Type-mix axis (T):** every operand slot must be exercised across these
  receiver/sender kinds, in both directions where applicable:
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Edge values per kind:** empty, min, max, overflow-by-one, all-spaces,
  all-zeros, sign at LEADING/TRAILING [SEPARATE], P-scaled, V-implied point.
- For each verb capture: result value(s), **FILE STATUS / special registers**
  (`RETURN-CODE`, `TALLY`), overflow/exception branch taken, and unchanged-on-error.

---

## Part A — DATA DIVISION sections (untested behaviors)

### WORKING-STORAGE SECTION
- **Levels:** 01, 02–49 nesting, 77 (independent), 66 `RENAMES a THRU b`, 88.
- **PIC:** `X A 9 S V P` with `(n)`; `P`-scaling (left/right); `V` implied point;
  edited combinations; `PIC` vs no-PIC group.
- **USAGE:** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2, COMP‑3/PACKED‑DECIMAL,
  COMP‑5, INDEX, POINTER — declaration + storage size + value round-trip.
- **VALUE:** numeric, signed, alphanumeric, figurative, `ALL "x"`; VALUE on group;
  illegal VALUE (size > PIC).
- **OCCURS:** fixed; `DEPENDING ON`; `INDEXED BY`; `ASCENDING/DESCENDING KEY`;
  multi-dimension (2–3); OCCURS on group.
- **Clauses:** REDEFINES (same/smaller/larger, chained), RENAMES, JUSTIFIED RIGHT,
  BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`, SYNCHRONIZED, FILLER.
- **88 condition-names:** single value, value list, `VALUE a THRU b`, multiple
  ranges, on numeric / alphanumeric / edited host; evaluation + `SET … TO TRUE`.
- **Initialization:** default (spaces/zeros by class) vs VALUE; **persistence
  across PERFORM and across CALL** (WS keeps last value).

### LOCAL-STORAGE SECTION
- **Re-initialized on every program entry** (contrast with WS persistence).
- VALUE clauses **re-applied each entry**.
- **Recursion:** each (recursive) CALL gets an independent LOCAL-STORAGE instance.
- Same clause coverage as WS (OCCURS/REDEFINES/88/…) but verify re-init semantics.

### LINKAGE SECTION
- Items have **no storage until bound** by the caller; accessing unbound linkage.
- Bound via `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (caller sees changes) vs **BY CONTENT** (callee edits a copy)
  vs **BY VALUE** (scalar).
- Group + elementary, OCCURS, REDEFINES, 88 in linkage.
- Size/USAGE mismatch between actual and formal parameter (behavior to observe).
- `ADDRESS OF` / `SET ADDRESS OF … TO` and POINTER binding (if supported).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — positional binding to CALL args; count
  mismatch (fewer/more args); order.
- Per-parameter `BY REFERENCE | BY VALUE` on the USING list.
- `RETURNING dn` — value handed back to `CALL … RETURNING`; vs `GIVING`; vs
  `RETURN-CODE`.
- Main program `USING` bound from the command line (if supported).
- Type mix on every parameter slot (apply **T**).

---

## Part B — Verb permutation matrix

Drive each verb across **T** for every operand slot. Below lists the *structural*
permutations (clauses/phrases) on top of the type mix.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (multiple receivers).
- `MOVE CORRESPONDING g1 TO g2` (matching elementary by name).
- Reference-modified source/target: `MOVE a(s:l) TO b(s:l)`.
- Subscripted: `MOVE t(i) TO u(j)`, `t(i,j)`.
- Type conversions (apply **T** both ways): num→edited, edited→num, alnum→num,
  num→alnum (justify/pad/truncate), group→group (byte copy), signed handling,
  COMP‑3↔DISPLAY, float↔fixed, figurative→each kind.

### DISPLAY
- `DISPLAY {dn|literal} …` (concatenated operands).
- `[WITH NO ADVANCING]`; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Screen form (observe/decide): `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Type mix: numeric (full PIC width), edited, signed, group, figurative.

### ACCEPT  *(spec all forms; many are screen/terminal — flag for scope decision)*
- `ACCEPT dn` (from console into alnum / numeric / edited / group).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Screen forms: `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Receiving into numeric vs numeric-edited vs alnum (de-edit / validation).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Multiple receivers each with its own ROUNDED/size behavior; mixed USAGE
  operands (COMP‑3 + DISPLAY + edited); signed; reference-modified operands.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- Divide-by-zero → ON SIZE ERROR; REMAINDER sign/scale; mixed USAGE.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Operators `+ - * / **`, parentheses, precedence; intrinsic functions in expr;
  mixed-USAGE operands; multiple receivers; truncation vs ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — nesting, empty branches, `NEXT SENTENCE`.
- Conditions: relation (`= < > <= >= NOT`), class (`IS [NOT] {NUMERIC|ALPHABETIC|
  ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), sign (`POSITIVE|NEGATIVE|ZERO`),
  88-condition reference, combined (`AND/OR/NOT`), **abbreviated** (`a = b OR c`),
  parenthesized.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` with
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Type mix in comparisons (num vs alnum vs edited vs figurative).

### PERFORM
- Out-of-line `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = literal / data-item).
- `PERFORM … UNTIL cond` with `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- Inline `PERFORM … END-PERFORM` (with TIMES/UNTIL/VARYING).
- Nested/recursive PERFORM; range overlap; index vs numeric loop var.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`; `GO TO p1 p2 … DEPENDING ON dn` (in/out of range).
- `CONTINUE`; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (from main vs subprogram).

### SET
- `SET index TO {n|index}`; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}`; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (where supported).

### INITIALIZE
- `INITIALIZE dn …` (group/elementary; default by category).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]`; tables (all occurrences).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (serial).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH` (binary;
  requires `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Found/not-found; multiple WHEN; key type mix; unsorted-table behavior.

### STRING  *(exercise the user's permutation style)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutations to cover:
  - single source `DELIMITED BY SIZE` → alnum target.
  - multiple sources, **mixed delimiters**: `STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`.
  - many sources/delims: `STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` start/advance; pointer out of range → overflow.
  - target too small → `ON OVERFLOW`; `NOT ON OVERFLOW`.
  - **type mix sources:** numeric, numeric-edited, signed, group, figurative,
    reference-modified — observe how each is stringified.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutations: single vs multiple delimiters, `ALL` (collapse repeats), `OR`,
  `DELIMITER IN`/`COUNT IN` capture, POINTER, TALLYING, more fields than data
  (overflow), targets of mixed type (numeric receivers get de-edited).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combined).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- BEFORE/AFTER scoping; overlapping matches; multi-char patterns; type-mix host.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Static (literal) vs dynamic (data-name) program name; unresolved → ON EXCEPTION.
- Arg passing modes (observe caller-visibility); arg count/type mismatch.
- `RETURNING` vs `RETURN-CODE`; recursion; `CANCEL prog`; `EXTERNAL` shared data.

### ARITHMETIC special-registers & misc verbs
- `ADD/SUBTRACT … GIVING` zero-suppression vs `TO` accumulation.
- `MOVE`/arith to/from `RETURN-CODE`, `TALLY`.
- `ALTER` (legacy GO TO) — observe / decide deprecate.
- `ACCEPT/DISPLAY` round-trip through edited fields.

### File verbs — *(only the gaps not in the file-I/O suite)*
- `OPEN … {SHARING WITH …|LOCK MODE …}`, `READ … {WITH [NO] LOCK}`,
  `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  with reference-modified keys; `UNLOCK`; multiple FDs sharing a record area.

### Planned verbs (spec for when implemented)
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- `RELATIVE` org: `READ/WRITE/REWRITE/DELETE/START` by `RELATIVE KEY`.

---

## Part C — Cross-form equivalence harness

For a curated set of the programs above, assert **identical** observable output
(DISPLAY text, FILE STATUS, RETURN-CODE, file contents) across three execution
forms of the same source:

1. **Interpreter** (`Interpreter::run`).
2. **AST round-trip** — serialize (`bincode`+`flate2`) → deserialize → run; assert
   byte-identical AST and identical output.
3. **Packed/compiled binary** — `cobolt_compiler::build_project` → execute the
   produced binary; assert identical output.

Any divergence between forms is a defect to log (the "one compiler, one behavior"
invariant).
