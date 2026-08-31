# Spec 054 — String methods on alphanumeric data items and literals

**Status:** requirements agreed and `/clarify` complete (operator, 2026-08-31).
All four open questions are answered below (D6-D9); the next phase is `/plan`.
**Classification:** NEW FEATURE (a capability beyond COBOL-85) → `features`
branch, its own commit, announced on forum f=96. It is **not** a fix and must
never share a commit with one (GOLDEN RULE #5).

---

## 1. The requirement

A `PIC X(n)` / `PIC A(n)` data item and an alphanumeric literal gain **methods**,
reached through the existing `::` member operator and chainable:

```cobol
DISPLAY WS-CSV-LINE::SPLIT(",")?::(1)::TRIM()
```

Read left to right: split the item on commas, take the first element of the
resulting array, trim its leading and trailing spaces, display the result.

A chain that produces an array fills a COBOL table:

```cobol
01 MY-COBOL-ARRAY OCCURS 10.
   05 WORDS PIC X(20).

MOVE SPACES TO MY-COBOL-ARRAY
MOVE "Hello, world, is, a, classic, example"::SPLIT(",")::TRIM()
  TO MY-COBOL-ARRAY
```

leaves `WORDS(1) = "Hello"`, `WORDS(2) = "world"`, `WORDS(3) = "is"`,
`WORDS(4) = "a"`, `WORDS(5) = "classic"`, `WORDS(6) = "example"`, and slots 7-10
unchanged (spaces, from the preceding `MOVE SPACES`).

**The COBOL-85 Nucleus is not modified.** No standard verb changes meaning, no
standard clause is redefined. The extension lives in the *expression*, plus the
receiving rules below.

---

## 2. What already exists (do not rebuild it)

Verified in the tree on 2026-08-31 — the operator and the chain are **already
implemented** for control objects, and this feature extends the receiver set
rather than inventing syntax:

| Piece | Where | State |
|---|---|---|
| `::` tokenized (two `Token::Colon`) | `cobolt-lexer/src/lexer.rs` | done |
| Member word after `::` reclassified out of keywords (so `::DELETE()` works) | `lexer.rs::reclassify_member_words` | done |
| `Expr::Member { recv, member, args, parens, span }`, left-recursive chain | `cobolt-ast/src/expr.rs` | done |
| Chained calls + subscripts (`Grid::Rows(I)::Cols(2)::Value`) | parser + interpreter | done |
| `Stmt::Invoke` — a trailing-call chain as a statement | `cobolt-ast/src/stmt.rs` | done |

**Genuinely new work:** a literal / alphanumeric data item as the chain *root*;
the bare `::(n)` index step; the `?` operator; the method library; and array
materialization into `OCCURS` / ODO tables.

⚠️ `Expr` is bincode-serialized into every compiled binary. **New variants are
appended, never inserted** — see the warning comment in `expr.rs`.

---

## 3. Settled design decisions (operator, 2026-08-31)

### D1 — Filling a table: **MOVE and SET both accepted**
`MOVE <chain> TO <table>` is the documented form; `SET <table> TO <chain>` is
accepted as a synonym when the receiver is a table. MOVE already targets group
items and tables, so no COBOL-85 statement changes meaning.

### D2 — **`DISPLAY` renders an array one element per line**
`DISPLAY "a,b,c"::SPLIT(",")` writes three lines. A scalar chain displays as one
line, exactly as `DISPLAY` always has.

### D3 — Element access is the bare **`::(n)`**
`::SPLIT(",")::(1)` is the first element. The grammar's "a name always follows
`::`" rule is extended to admit a nameless subscript step. **Subscripts are
1-based**, as COBOL counts everywhere else. An out-of-range index yields the
null string (see D4's result rule), never an abort.

### D4 — **`?` short-circuits the whole chain**
`?` may follow any method or property step. If the marked step yields nothing —
no match, an empty array, an index past the end — the **entire chain stops** and
the result is the null string (spaces, to the receiver's declared length).
Nothing after the `?` runs. This is the closest honest analogue of Rust's early
return, and it is what makes `SPLIT(",")?::(1)` safe on a line with no comma.

Without `?`, a step that yields nothing is a runtime error the program can trap.

### D5 — Method set: **the COBOL-safe subset, plus in-place mutation**
Implemented: inspection, searching, iteration/splitting, transformation,
slicing — **and** the buffer-mutation group, redefined to operate on the
receiving item *within its declared PIC length*, truncating or space-padding at
the boundary.

**Excluded** (no meaning for fixed-length COBOL storage, or unstable in Rust):
`as_ptr`, `as_bytes`, `get_mut`, `reserve`, `shrink_to_fit`, `into_boxed_str`,
`floor_char_boundary`, `ceil_char_boundary`. `into_bytes` is **deferred** — it
yields a byte vector with no COBOL receiver, and the operator's own note limits
it to `PIC X`; it needs its own ruling before it is built.

### D6 - `LEN` reports BYTES, not characters
Storage-true, matching the `PIC X(n)` declaration and `FUNCTION LENGTH`. A
caption holding accented UTF-8 therefore reports more than its visible
character count, and that is the honest answer for fixed-length COBOL storage:
every slicing and mutation offset is a byte offset, so a character-counting
`LEN` would disagree with the very methods it is used to drive.

### D7 - Array overflow TRUNCATES SILENTLY
A chain yielding more elements than the receiving table has slots fills what
fits and drops the rest. This is what `MOVE` already does to an oversized
alphanumeric source, so the extension introduces no new failure mode and no
new status register.

### D8 - A mutation method on a LITERAL receiver is a compile-time error
`"abc"::PUSH("d")` has nothing to write to. The semantic analyser rejects it
with a diagnostic naming the method and the literal; it is never a runtime
error, because it is always statically decidable.

### D9 - `GET` is DROPPED; reference modification is the way
COBOL-85 reference modification `item(start:len)` already takes a substring and
is the form a COBOL developer already knows. `GET(start,len)` would have been a
second spelling of it, so it is not implemented. The guide documents reference
modification as the substring mechanism; `SPLIT-AT(i)` stays, because splitting
into two parts is not something reference modification expresses.

---

## 4. Naming — COBOL words, not Rust identifiers

Method names are COBOL words: **letters, digits and hyphens, case-insensitive,
no underscores**. `split_whitespace` is `SPLIT-WHITESPACE`, `to_lowercase` is
`TO-LOWERCASE`, `char_indices` is `CHAR-INDICES`.

`LEN` is deliberately **not** `LENGTH`: `LENGTH` is a COBOL-85 intrinsic and an
`ON`-phrase keyword, and shadowing it would modify the Nucleus.

| Group | COBOL name | Returns |
|---|---|---|
| Inspection | `LEN` · `IS-EMPTY` · `IS-CHAR-BOUNDARY(i)` | number · boolean · boolean |
| Searching | `CONTAINS(p)` · `STARTS-WITH(p)` · `ENDS-WITH(p)` · `FIND(p)` · `RFIND(p)` | boolean · boolean · boolean · position · position |
| Iteration | `CHARS` · `CHAR-INDICES` · `BYTES` · `LINES` · `SPLIT-WHITESPACE` · `SPLIT(p)` · `RSPLIT(p)` · `SPLIT-TERMINATOR(p)` · `MATCHES(p)` · `RMATCHES(p)` | **array** |
| Transformation | `TO-LOWERCASE` · `TO-UPPERCASE` · `REPEAT(n)` · `REPLACE(from,to)` · `REPLACE-N(from,to,count)` · `TRIM` · `TRIM-START` · `TRIM-END` · `TRIM-MATCHES(p)` | string |
| Slicing | `SPLIT-AT(i)` | array of 2 |
| Mutation (in place) | `PUSH(c)` · `PUSH-STR(s)` · `INSERT(i,c)` · `INSERT-STR(i,s)` · `CLEAR` · `TRUNCATE(n)` · `POP` · `REMOVE(i)` · `REPLACE-RANGE(start,len,s)` | — (writes the receiver) |

**Positions are 1-based and 0 means "not found"** — the convention `INSPECT`,
`UNSTRING` and `FUNCTION` already use. A Rust byte offset is never surfaced raw.

---

## 5. Clarify - resolved (operator, 2026-08-31)

All four questions this spec opened are answered, and each answer is recorded
as a decision above. Nothing here is left for `/plan` to guess.

| # | Question | Answer | Decision |
|---|---|---|---|
| 1 | Does `LEN` count bytes or characters? | **Bytes** (storage-true) | D6 |
| 2 | Array yields more elements than the table holds | **Truncate silently** | D7 |
| 3 | Mutation method on a literal receiver | **Reject at compile time** | D8 |
| 4 | `GET(start,len)` vs reference modification | **Drop `GET`**, document refmod | D9 |

## 6. Acceptance

- Both worked examples in §1 produce exactly the stated results, as tests.
- `?` short-circuit proven on a line with no delimiter.
- 1-based indexing and `0 = not found` proven per search method.
- Existing `Grid::Rows(I)::Cols(2)::Value` object chains keep working unchanged.
- No COBOL-85 conformance regression: the full NIST CCVS85 census stays
  **420/420, 8362 assertions PASS / 0 FAIL** (GOLDEN RULE: a NIST fix or any
  language change requires the full regression, not a spot check).
