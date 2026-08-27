# Plan — separator comma, separator semicolon, space-separated subscripts

- **Spec:** [NIST-spec-separators.md](NIST-spec-separators.md)
- **Status:** approved
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-26
- **Classification:** fix (technical debt — a COBOL-85 construct that should
  already work). Forum f=97.

## 1. Approach

COBOL-85 defines a **separator comma** and a **separator semicolon** as a comma
or semicolon *followed by a space* (or end of line). They are pure decoration:
they may appear anywhere a space may appear and they mean exactly what a space
means. The complement is what makes this tractable — a comma **not** followed by
a space is never a separator, and that is precisely where the two constructs
that need a comma live:

| Written | Comma is | Because |
|---|---|---|
| `MOVE ZERO TO DN3, DN4.` | a separator | followed by a space |
| `1,5` under `DECIMAL-POINT IS COMMA` | a decimal point | glued between digits |
| `PIC ZZ,ZZ9` | an editing character | glued inside the template |

So the rule is one-sided and cheap to state: **drop `,` and `;` when the next
character is whitespace or end of input; keep them otherwise.** Nothing that
currently consumes a comma loses it, because nothing that currently consumes a
comma has a space after it.

### Where it goes — the lexer, per spec Q1

The spec's Q1 recommends the lexer over the parser, and the enumeration
demanded by Q2 confirms it. R3 lists nine syntactic sites; adding a `p.eat(&
Token::Comma)` to each is nine chances to miss one, and the list is not
exhaustive. One rule in `Lexer::next_token` covers every site that exists now
and every site added later.

The skip happens **before** `classify()`, so a dropped separator never touches
`at_line_start` or any other lexer state. It is not "a token the parser
ignores"; it never becomes a token at all, which is what "means what a space
means" actually says.

### Q2 answered — every existing consumer, enumerated

`Token::Semicolon` has **zero** consumers in the whole workspace: it is produced
by the lexer and matched by nothing. Dropping the separator form costs nothing.

`Token::Comma` has thirteen. They fall into four groups:

| Group | Sites | Effect of the change |
|---|---|---|
| **Decimal comma** | `expr.rs:133,141,197` | **unaffected** — all three already require `glued()`, and a glued comma is never a separator. |
| **PICTURE template** | `data.rs:545,582` | **unaffected** — `PIC ZZ,ZZ9` is glued. |
| **Optional eats in RParen/keyword-bounded loops** | `expr.rs:297`, `stmt.rs:2097`, `procedure.rs:180`, `data.rs:829,839,886` | become no-ops; the loops were already bounded by something other than the comma. |
| **Loops bounded BY the comma** | `expr.rs:349,431,467`, `stmt.rs:2633` | **would break** — see below. |

That last group is the whole risk of this change, and it is also where R4 lives.

### The four comma-bounded loops

Two are COBOL subscript lists:

```rust
let mut indices = vec![first];
while p.eat(&Token::Comma) {        // ← stops the moment the comma is gone
    indices.push(parse_expr(p));
}
p.expect(&Token::RParen);
```

With separators dropped, `TABLE (1, 2)` arrives as `TABLE (1 2)`, one index is
read and `expect(RParen)` fails on the `2` — which is *exactly* the
`expected RParen, found …` diagnostic R4 already reports for the space-separated
form the suite actually writes. Rebinding the loop to the closing parenthesis
fixes the regression and delivers R4/R5 in the same edit; the comma becomes
optional punctuation inside the list rather than its delimiter.

The other two are RustCOBOL's own `::method(a, b)` argument lists — **not**
COBOL-85, but real syntax that must not regress. They get the same treatment,
which also makes them consistent with the `FUNCTION` argument loop at
`expr.rs:297` that already reads this way.

Every rebound loop carries a **liveness guard** (`if p.pos == before { break }`),
copied from `expr.rs:297`: an argument that consumes nothing must end the loop
rather than spin, since the comma is no longer guaranteed to advance it.

### What is deliberately NOT done

- `EXEC RUST … END-EXEC` is captured verbatim by `try_capture_exec_rust` as a
  single `ExecRustBlock` token. Its Rust semicolons are never lexed as COBOL and
  are untouched. Confirmed by reading the capture, not assumed.
- A comma with no space after it (`MOVE A,B`) keeps its token. The standard does
  not call that a separator, and the optional eats above still absorb it, so the
  lenient reading survives with no new rule (R8).

## 2. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-lexer/src/lexer.rs` | the separator rule in `next_token`, plus `is_separator_punctuation()` |
| `crates/cobolt-parser/src/expr.rs` | rebind three comma-bounded loops (two subscript, one member-arg) |
| `crates/cobolt-parser/src/stmt.rs` | rebind the `::method(…)` argument loop |
| `crates/cobolt-lexer/tests/test_literals.rs` | separator vs decimal vs PICTURE lexing |
| `crates/cobolt-parser/tests/` | AC1-AC6 parse-level guards |
| `docs/cobol85-supported-syntax-en.md` | separators note + re-measured scoreboard |

## 3. Data / model changes

None. No AST node, token variant or public signature changes. `Token::Comma`
and `Token::Semicolon` both remain in the token enum and both are still produced
for the non-separator spellings.

## 4. Key decisions & alternatives

- **Lexer, not parser** (spec Q1's recommendation). Rejected alternative: a
  `skip_separators()` helper called at each of R3's sites — nine call sites,
  unbounded growth, and no coverage for a site nobody thought of.
- **"Followed by whitespace", not "not glued on both sides".** The stricter
  two-sided test would drop the comma in `PIC ZZ,ZZ9` only if a space preceded
  it, which never happens — but the one-sided test is the standard's own
  wording, so it is the one that will still be right for a case not yet seen.
- **Rebind the comma-bounded loops rather than special-case them.** Making the
  lexer keep commas inside parentheses would preserve those four loops
  untouched, but it would also keep failing R4, which is 11 of the 51 programs.

## 5. Risks & mitigations

| Risk | Mitigation |
|---|---|
| A comma-bounded loop is missed and silently truncates a list | All four found by exhaustive `grep` for `Token::Comma`, tabulated above; each gets a test |
| `parse_expr` greedily eats the next subscript (`(A B)` → one expression) | AC4/AC5 assert the index **count**, not just that it parses |
| Decimal comma regresses | AC8 runs `DECIMAL-POINT IS COMMA` end to end; the three `glued()` sites are untouched by construction |
| A rebound loop spins on an unparseable argument | Liveness guard on every one, matching `expr.rs:297` |

## 6. Test strategy

Guard tests are written **before** the change and must fail against unmodified
code — the handoff's standing lesson. Lexer level: a separator comma produces no
token; a glued comma still does; `PIC ZZ,ZZ9` keeps its comma; a semicolon
separator vanishes. Parser level: AC1-AC6 as parse assertions on index and
argument counts. Regression: the full workspace sweep, `--no-fail-fast`, reading
every `test result:` line.

Measurement is the NIST harness, re-run before and after:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

## 7. Steering compliance

- **PRIME DIRECTIVE:** Rust only. No script is used to make or check these edits.
- **i18n:** no user-facing IDE string changes.
- **Generated code:** the RAD generator emits no separator commas, so generated
  `.cbl` output is byte-identical. Guarded by the existing codegen suite.
- **Version:** `z` bump only.
- **Docs:** `docs/cobol85-supported-syntax-en.md` is the registry-mapped document
  for language coverage and is updated with a re-measured scoreboard.

## 8. Open questions

Both of the spec's are now closed:

- **Q1 (lexer or parser?)** → **lexer**, as recommended, with Q2's enumeration
  supporting it.
- **Q2 (what else uses these tokens?)** → answered in §1; `Token::Semicolon` has
  no consumers, `Token::Comma` has thirteen in four groups, and only the four
  comma-bounded loops need code changes.
