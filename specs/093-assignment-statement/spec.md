# Spec — COMPUTE without its verb: `A = B * C`

- **Status:** draft — feasibility proven by a spike; no decision taken on the open questions (§7)
- **Folder:** specs/093-assignment-statement/
- **Author:** operator request, written by Claude   **Date:** 2026-10-12
- **Number:** the operator asked for 092; that number is spec 092 *PowerCloud*, so this is the next free one, 093.
- **Spike:** branch `features-assignment-statement`, commit `6b01f79` (parser only, not versioned, not for merge). §9 says how to pick it up.

## 1. Overview
`COMPUTE` is the one arithmetic verb whose verb adds nothing: its form is
`COMPUTE receiver [ROUNDED] = expression`, and the `=` already says it. The
operator wants to write what every other language writes:

```cobol
A = B * C
A ROUNDED = B * C
    ON SIZE ERROR …
    NOT ON SIZE ERROR …
END-COMPUTE
```

and have the compiler supply the verb, so that the interpreter, the debugger and
a running program all see

```cobol
COMPUTE A = B * C  [ON SIZE ERROR …] [NOT ON SIZE ERROR …] END-COMPUTE
```

with **no change to the interpreter or the runtime**. This spec records what was
found when that was tried: it is feasible, it is a parser change of about 75
lines, and what remains is the tools that read source text rather than the
parse tree (§8).

## 2. Goals / Non-goals
- **Goals**
  - `receiver [ROUNDED] = expression` is a statement, with the size-error
    phrases and `END-COMPUTE` that `COMPUTE` has.
  - It is the **same statement** as `COMPUTE`: one AST node, so run, debug,
    `rcrun`, the compiled binary and every analysis behave identically.
  - No program that parses today parses differently.
  - The IDE's text-reading tools (Beautify, breakpoints, flagging) know the new
    spelling, so it is not a second-class statement.
- **Non-goals**
  - No change to the interpreter, the runtime, `cobolt-semantic`'s rules,
    `cobolt-codegen`, `cobolt-compiler` or the serialized AST.
  - No new keyword, no new token, no new AST node.
  - `COMPUTE` stays valid, and stays the only way to write several receivers.
  - No assignment of **text**: `A = "x"` is not `SET A TO "x"` (Q3).
  - No text-level preprocessing pass that rewrites source into `COMPUTE` (§10).

## 3. User stories
- As a developer, I want to write `TOTAL = PRICE * QTY` and have it mean
  COMPUTE, so that arithmetic reads as arithmetic.
- As a developer, I want `ROUNDED` and `ON SIZE ERROR` to work on it exactly as
  they do on COMPUTE, so that I lose nothing by dropping the verb.
- As a developer debugging, I want to set a breakpoint on `A = B * C` and step
  through it, so that the shorthand is not a statement I cannot stop on.
- As a developer using Beautify, I want the shorthand laid out like any other
  statement, so that the style rules still hold.
- As a developer with an existing program, I want nothing in it to change.

## 4. Requirements (EARS)

### 4.1 The statement
- **R1 (event):** When a statement opens with a receiver followed by `=`, the
  parser shall build the `Stmt::Compute` that `COMPUTE receiver = …` builds,
  with the receiver as its single target and `rounded` false.
- **R2 (ubiquitous):** A receiver shall be a data name, optionally qualified
  with `OF`/`IN`, subscripted or reference-modified in parentheses
  (`F OF G`, `T(I J)`, `S(1:4)`), or a control's member (`Ctrl::Property`,
  `Ctrl::Items(3)`).
- **R3 (event):** When `ROUNDED` follows the receiver and is itself followed by
  `=`, the target shall be rounded.
- **R4 (event):** When the expression is followed by `ON SIZE ERROR` and/or
  `NOT ON SIZE ERROR`, the parser shall take the phrases, and `END-COMPUTE` if
  present, exactly as `COMPUTE` does. A phrase ends at `END-COMPUTE`, at the
  period of the sentence, or where `COMPUTE`'s phrase would end.
- **R5 (constraint):** The shorthand shall take one receiver. `COMPUTE A B = X`
  stays the only way to write several.

### 4.2 Neighbours
- **R6 (ubiquitous):** An operand list — `DISPLAY A B`, `MOVE A TO B C`,
  `INITIALIZE G H`, `CANCEL "P"`, the receivers of `ADD`/`SUBTRACT`/`MULTIPLY`/
  `DIVIDE`, `STRING`, `UNSTRING`, `CALL … USING`, `INSPECT … TALLYING`,
  `SET` — shall end in front of an assignment, as it already ends in front of a
  method call, so the identifier on the next line is a new statement.
- **R7 (constraint):** `ROUNDED` alone shall not end an operand list: in
  `ADD 1 TO A ROUNDED B ROUNDED` it follows a receiver (§9, collision).
- **R8 (constraint):** The shorthand shall not change the parse of any program
  that parses without it. The measure is the NIST CCVS85 `strict` pass and the
  repository's own test suites (§5).
- **R9 (constraint):** `COMPUTE`'s own receiver list shall still stop at `=`
  only, so `COMPUTE A B = X` keeps its two receivers.

### 4.3 What stays untouched
- **R10 (constraint):** The interpreter, runtime, semantic rules, code
  generator, compiler and serialized AST shall not change. A program written
  with the shorthand and the same program written with `COMPUTE` shall produce
  the same AST apart from source positions, and the same output.

### 4.4 Diagnostics
- **R11 (state):** While the receiver is not numeric, the existing check
  ("'A' is not numeric; COMPUTE requires a numeric receiver") shall report it.
  Its wording is Q3.
- **R12 (event):** When a line cannot be read as a statement (`A B = C`, `A + 1`),
  the parser shall report what it reports today.

### 4.5 Tools that read text
- **R13 (ubiquitous):** Beautify shall treat a shorthand line as the opening of a
  statement of the `COMPUTE` family: its indentation, its size-error phrases
  (one level in) and its `END-COMPUTE` (aligned with it), its blank-line rules
  (verb blocks, comments) and its period rules, with the parse-tree check
  unchanged. It shall not lay the line out as a continuation of the line above.
- **R14 (ubiquitous):** The debugger shall accept a breakpoint on a shorthand
  line and stop on it.
- **R15 (ubiquitous):** The conformance flagging that reports `COMPUTE` as a
  statement shall report the shorthand the same way. (Q6)
- **R16 (optional):** Where the operator chooses it (Q4), Beautify's style shall
  drop the verb — `COMPUTE A = B * C` → `A = B * C` — only when the parse-tree
  check proves the program unchanged, as it does for `MOVE` → `SET`.

### 4.6 Documentation and agents
- **R17 (ubiquitous):** The Developer's Guide (*Code style*, §13) shall describe
  the shorthand, and the support matrix shall list it as a RustCOBOL extension —
  it is not COBOL-85.
- **R18 (optional):** Where Q4 is answered "prefer the shorthand", the plugin's
  `layout` golden rule and `APPLICATION_RULES` shall say so, with the model
  updated.

## 5. Acceptance criteria
Baselines are measured on `main` at 1.91.14 with the spike applied.

- [ ] AC1 — R1–R5: parser tests for a plain receiver, `ROUNDED`, the phrases and
  `END-COMPUTE`, a qualified, a subscripted and a member receiver, and a run of
  consecutive assignments. *(Spike: 8 tests, `test_assignment_statement.rs` in
  `cobolt-parser`, passing.)*
- [ ] AC2 — R6, R7, R9: a test per neighbouring verb that an operand list ends in
  front of an assignment, and that `ADD`/`MULTIPLY` receiver series and
  `COMPUTE`'s two receivers are unchanged.
- [ ] AC3 — R8: `cobolt-parser` — its 190 existing tests pass, with the 8 new ones beside them; `cobolt-runtime`,
  `cobolt-semantic`, `cobolt-ast`, `cobolt-lexer`, `cobolt-codegen` 1,800 pass,
  0 fail, 12 ignored; **NIST `strict`: 459 programs analysed, 441 clean, 420 of
  420 in-scope pass, 18 failing — the same 18, with the same diagnostics** (a
  line-for-line diff of the summary against `main`). *(Spike: met.)*
- [ ] AC4 — R10: a COBOL program run once with `COMPUTE ` and once without, over a
  template that differs only in that word, displays the same lines, and the
  lines are the right ones (the receiver, `ROUNDED`, a size error, a subscript,
  a qualified name, `IF`/`ELSE`, `PERFORM`, assignments after `DISPLAY`, `MOVE`
  and `ADD`). *(Spike: 10 lines, equal, passing, in
  `cobolt-runtime/tests/test_assignment_statement.rs`.)*
- [ ] AC5 — R13: Beautify tests: a shorthand line indents as a statement; its
  `ON SIZE ERROR` and `END-COMPUTE` align; a second pass changes nothing; the
  parse-tree gate still reports every block unchanged over PowerSpatial,
  PowerDemo3 and PowerChat after the new spelling is used in them.
- [ ] AC6 — R14: a breakpoint set on a shorthand line shows its ring and the
  debuggee stops there (a test beside `test_debug_breakpoints.rs`, and the
  line-rule unit test in `debugger.rs`).
- [ ] AC7 — R15, R17, R18: flagging test; Guide and matrix updated (English
  canonical; its translations deleted, Golden Rule 8); the plugin text and its
  lint test, if Q4 says so.
- [ ] AC8 — Golden Rule 7: any new `tests/cobol` program prints a quantified
  result block — the cases it covers and `TESTS RUN` / `TESTS FAILED` /
  `OVERALL RESULT`.

## 6. Constraints & steering check
- **i18n:** no new IDE string unless Q3 adds a diagnostic with its own wording
  (then six languages).
- **Generated code:** unchanged. Codegen keeps writing `COMPUTE` where it writes
  arithmetic; the contract "generated COBOL is read-only" is untouched.
- **Docs:** the Developer's Guide *Code style* section and the support matrix
  change (R17). Translations of the Guide are deleted, not patched (Golden Rule
  3 and 8).
- **System KB:** a source of Rust constants (`cobolt-compiler`), not the guide;
  if the KB's language reference lists statements, it needs the shorthand, and
  `chunked.data` must be regenerated in the same change. Check before closing.
- **Fix vs feature:** **feature** (Golden Rule 5) — a non-standard language
  extension, beyond COBOL-85. `features` branch, a `z` bump and a changelog
  entry, announced on f=96 only when the operator asks (Rule 4b).
- **NIST:** no behaviour may cost the suite an assertion (operator, 2026-09-06).
  The shorthand is outside the standard, so the suite never uses it; AC3 proves
  it does not disturb the suite.
- **Standing rule "COBOL you write uses the RustCOBOL extensions"** (2026-10-09):
  if this lands, the shorthand joins the list in `CLAUDE.md`, the guide and the
  plugin rule — Q4.

## 7. Open questions
- **Q1 — `END-COMPUTE`.** With a size-error phrase, a phrase body runs to
  `END-COMPUTE` or the period, as in `COMPUTE`. The operator's two examples show
  the phrases without a terminator. *Recommended:* keep COBOL-85's rule —
  `END-COMPUTE` closes it, a period also does — which is what style rule 7
  already asks for. The alternative, *requiring* `END-COMPUTE` when a phrase is
  present, is a stricter rule than `COMPUTE`'s and would be the shorthand's only
  difference from it.
- **Q2 — several receivers.** `A B = X` could be allowed, but `A` at the start of
  a statement followed by a name has no other reading today, and keeping the
  shorthand to one receiver keeps the lookahead trivial. *Recommended:* one.
- **Q3 — text and the message.** `A = "x"` on a `PIC X` item is a COMPUTE, so
  the check says "COMPUTE requires a numeric receiver" about a line with no
  COMPUTE in it. Options: leave it; reword it ("an arithmetic assignment
  needs…"); or make `A = expr` mean `SET` when the receiver is text. The last
  needs the receiver's type at the point of the statement — the semantic pass has
  it, the parser does not — and is a different feature (it changes what the
  interpreter is asked to do). *Recommended:* reword only.
- **Q4 — who prefers the shorthand.** Should Beautify's style drop the verb
  (R16), and should the plugin tell an agent to write the shorthand (R18)? Both
  are the operator's style call, as `MOVE` → `SET` was.
- **Q5 — `Ctrl::Property = expr`.** It parses, because a COMPUTE receiver may be a
  member. Whether the runtime stores to it as it would for `COMPUTE`, and whether
  it is to be documented at all, was not checked; it is *not* the property
  assignment `SET Ctrl::Property TO` that the style prefers.
- **Q6 — flagging.** The standard-flagging pass reports `COMPUTE` as a COBOL-85
  statement; the shorthand is not COBOL-85 at all. Report it as `COMPUTE`, or as
  an extension?
- **Q7 — names.** "Assignment statement" (this spec's folder) or "implicit
  COMPUTE"? It only matters for the Guide's heading and the changelog.

## 8. The surfaces that read text, not the tree

The parser is one place. These are the others — everything that decides what a
line *is* by looking at its words — found by searching the workspace for
`COMPUTE` and for each statement-start rule.

| Surface | Where | Today with `A = B * C` | Needed |
|---|---|---|---|
| Parser, statement start | `cobolt-parser/src/stmt.rs` `parse_stmt` | `unexpected token in statement` | the spike (§9) |
| Parser, operand lists | `is_expr_start`, 18 call sites in `stmt.rs` | the next line's name is taken as one more operand | the spike |
| **Beautify** | `panels/beautify.rs` (`VERBS`), `panels/beautify_style.rs` (`STATEMENT_VERBS`, `statement_verb`, `phrase_scoped`) | laid out as a **continuation** of the line above (below) | R13 |
| **Debugger breakpoints** | `panels/debugger.rs` `COBOL_STMT_VERBS` (≈4170) and its use (≈4335): a line qualifies if its first word is a verb or `name::` | no ring; a click does nothing | R14 |
| **Standard flagging** | `cobolt-semantic/src/flagging.rs` ≈712, `matches!(st.token, Token::Compute)` | not flagged | R15 |
| Editor highlighting | `panels/editor.rs` ≈43, ≈274 (keyword lists) | the name is not a keyword; nothing to colour | cosmetic, none |
| Semantic pass | `resolver.rs` ≈296, `type_checker.rs` ≈130, `national.rs` ≈212 | work on `Stmt::Compute` | none |
| Interpreter | `interpreter.rs` ≈5611 `exec_compute` | runs `Stmt::Compute` | none |
| Code generator / compiler | write `COMPUTE`; serialize the AST | none | none |
| Value-method rewriter, IDE | `cobolt-ast/src/value_methods.rs` | token-aware, looks only after `::` | none |

What Beautify does today with the shorthand (probe, style on):

Input:

```text
MAIN-P.
IF A > 1
A = B * C
D ROUNDED = A / 3
ON SIZE ERROR
DISPLAY "X"
END-COMPUTE
COMPUTE E = A + 1
END-IF.
F = 1.
DISPLAY "Z".
```

Output with the PowerRustCOBOL style on:

```text
       MAIN-P.
           IF    A > 1
                   A = B * C
                   D ROUNDED = A / 3
                   ON SIZE ERROR
               DISPLAY    "X"
               END-COMPUTE
               COMPUTE    E = A + 1
           END-IF
               F = 1
           DISPLAY    "Z".
```

The statement lines hang under the one above, the phrase is not one level in,
and `END-COMPUTE` is not under its statement. Nothing about the meaning changes —
the parse-tree gate holds — but the layout is wrong, so R13 is needed before the
shorthand is used in an example.

## 9. The spike

**Where:** branch `features-assignment-statement`, commit `6b01f79`.
`git switch features-assignment-statement` picks it up; `git show 6b01f79` is the
whole of it.

**What it is** — `cobolt-parser/src/stmt.rs`, plus two test files:

1. `at_assignment(p)` — a lookahead that does not consume. From an identifier it
   skips any run of `OF`/`IN name`, a balanced `( … )`, and `:: member`, and then
   answers true on `=`, or on `ROUNDED` **followed by `=`**.
2. `parse_stmt` asks it first, before the inline-invoke arm, because
   `Ctrl::Property = …` also opens with `name ::`.
3. `parse_assignment` parses the receiver as an expression, eats `ROUNDED`, and
   calls `finish_compute` — the tail `parse_compute` now shares: `=`, the
   expression, the size phrases, `END-COMPUTE`, and the `Stmt::Compute`. One
   builder, so the two spellings cannot drift.
4. `is_expr_start` is split into `can_start_expr` (what it was) and
   `is_expr_start = can_start_expr && !at_assignment` — the one line that ends an
   operand list in front of an assignment, as `at_cobol_object_call` ends it in
   front of a `COBOL::` call. `parse_compute`'s own receiver loop uses
   `can_start_expr`, so `COMPUTE A B = X` is unchanged.

**Measured** (§5 AC1–AC4): parser 190 existing tests plus 8 new, the five other crates 1,800,
the NIST `strict` summary identical to `main` line for line, and one program in
both spellings giving the same ten lines.

**The collision, and what found it.** The first version accepted `ident ROUNDED`
alone. Two tests in `test_nist_grammar.rs` failed at once — the receiver series
`MULTIPLY a BY b ROUNDED c ROUNDED` and the `DIVIDE` equivalent — because
`ROUNDED` follows a receiver in every arithmetic verb, so `C ROUNDED` after
`B ROUNDED` read as a new statement. Requiring `=` after `ROUNDED` fixed it, and
is R7. It is the kind of thing the NIST grammar tests exist to catch, and it is
why AC3 reads the NIST summary and not only the unit tests.

**What is not in the spike:** Beautify, the debugger rule, flagging, the guide,
the plugin text, the version and the changelog (R13–R18, AC5–AC7).

## 10. Alternatives considered
- **A text pass before the lexer** that inserts `COMPUTE `. Rejected: it moves
  every later column on the line, so error positions, the debugger's line map
  and the editor's squiggles disagree with the file the developer sees; it must
  know a statement start from a condition, which is the parser's job; and it
  could not tell `ADD 1 TO A ROUNDED` from the shorthand without parsing.
- **A new `Stmt` variant** (`Assign`). Rejected: the interpreter, three semantic
  passes and the serialized AST would each need an arm for a statement that
  means exactly COMPUTE, and the debugger and the compiled binary would carry
  two spellings of one thing.
- **A runtime change** that evaluates `A = B * C` itself. Rejected by the
  brief, and unnecessary.
- **Requiring `END-COMPUTE`** — Q1.

## 11. Risks
- **A name that is a keyword.** A data name the lexer reads as a keyword token
  (`COUNT`, `DATE`) is not `Token::Identifier`, so `COUNT = 1` is not the
  shorthand and gives today's error. Same limit `MOVE` has for its operands.
  *Mitigation:* none needed; it is the existing reserved-word rule.
- **A typo reads differently.** `A B = C` and `A + 1` give today's
  "unexpected token" — R12 — but a one-word typo before `=` (`TOTL = P * Q`) is
  now a COMPUTE to an undeclared name, which the resolver reports as undeclared,
  not as a bad statement. *Mitigation:* the message already names the item.
- **Text targets** (Q3) are the shorthand's sharpest edge: `A = "x"` looks like
  assignment in every other language, and here it is an arithmetic error unless
  the message says why.
- **Style churn.** If Q4 says Beautify drops the verb, every example project and
  all golden snapshots change once, as they did for `SET`.
