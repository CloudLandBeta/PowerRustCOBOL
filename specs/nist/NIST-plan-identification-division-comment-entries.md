# Plan — IDENTIFICATION DIVISION comment-entry paragraphs

- **Status:** draft → approved
- **Spec:** [`NIST-spec-identification-division-comment-entries.md`](NIST-spec-identification-division-comment-entries.md)
  **Date:** 2026-08-25
- **Impact:** **38 of the 197 remaining in-scope failures** — the largest bucket
  at the 237 / 434 baseline. 32 fail on the comment-entry text itself
  (`expected Division, found …`), 6 on `DATE-COMPILED`
  (`expected PROCEDURE DIVISION` at end of file).

## 1. Approach

Three separate defects share one cause: **the parser does not know where a
comment-entry ends.** All three are fixed by replacing the termination rule in
`collect_comment_text` and widening the paragraph loop around it. Everything
lives in `crates/cobolt-parser/src/identification.rs`; no other crate is
touched.

### The measured defects

**(a) `DATE-COMPILED` is a keyword nobody handles.** It lexes to
`Token::DateCompiled` (`keywords.rs:58`) but the paragraph loop
(`identification.rs:73`) matches only `Author`, `DateWritten` and a generic
`Identifier`, so it falls through `_ => break`. The division ends early and the
parser then demands a division header where a paragraph name sits.

**(b) The entry terminates on a period.** `collect_comment_text`
(`identification.rs:110`) consumes to the next `Period`. A comment-entry is free
text and routinely contains periods — CM101M's `INSTALLATION` runs **nine
lines**, several ending in one.

**(c) Reserved words inside the text are treated as real.** Collection also
stops at `Environment`, `Data` and `Procedure`. CM101M line 000800 reads
`AUTOMATED DATA AND TELECOMMUNICATION SERVICE.`; collection stops at `DATA`, the
parser concludes the DATA DIVISION has begun, looks for `DIVISION`, finds `AND`.

### The rule that replaces all three

COBOL-85: a comment-entry occupies Area B of one or more lines and ends at **the
next entry beginning in Area A**. The parser cannot use Area A directly — by the
time it runs, fixed-format source has been flattened and re-tokenized as free
form, so it does not know which format the developer wrote. But `Span` carries
`line` and `col` (`span.rs:23`), and that is enough for a rule that works for
both:

> A comment-entry ends at the first token that **begins a new source line** and
> **has the shape of a paragraph or division header**.

Both halves are required, and each one alone is insufficient:

| Case | line-start? | header shape? | Result |
|---|---|---|---|
| `AUTOMATED DATA AND …` — `DATA` mid-line | no | no (no `DIVISION` after) | text ✓ |
| `SOFTWARE DEVELOPMENT OFFICE.` | yes | no | text ✓ |
| `5203 LEESBURG PIKE  SUITE 1100` | yes | no | text ✓ |
| `PHONE   (703) 756-6153` | yes | no | text ✓ |
| `DATE-WRITTEN.` | yes | yes | **ends the entry** ✓ |
| `SECURITY.` | yes | yes | **ends the entry** ✓ |
| `ENVIRONMENT DIVISION.` | yes | yes | **ends the entry** ✓ |

"Header shape" means one of:

- `Token::Author`, `Token::DateWritten`, `Token::DateCompiled`;
- `Token::Identifier(s)` where `s` is `INSTALLATION`, `SECURITY` or `REMARKS`,
  **followed by a `Period`**;
- `Token::Environment` / `Token::Data` / `Token::Procedure`, **followed by
  `Token::Division`**;
- `Token::Identification`, or `Token::Eof`.

The "followed by" checks are what make (c) safe: a bare `DATA` in prose is not a
division header, and a bare `SECURITY` in prose is not a paragraph name.

Requirement mapping: **R1** (all six paragraphs) and **R5** (any order, any
subset) come from widening the loop; **R2** and **R6** from the new termination
rule; **R3** and **R4** are both satisfied by the single line-start rule above,
which is *better* than the spec's two-rule proposal — see §4.

## 2. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-parser/src/identification.rs` | **the whole change.** Rewrite `collect_comment_text`'s termination; add a `starts_a_paragraph()` predicate; add `DateCompiled` and named-`Identifier` arms; populate `installation` / `date_compiled` / `security` (currently hard-coded `None` at line 100). ~60 lines. |
| `crates/cobolt-parser/tests/test_identification.rs` | extend — see §6 |
| `docs/cobol85-supported-syntax-en.md` | the six paragraphs are not documented at all; add them, and re-measure the **NIST scoreboard** (required by `specs/steering/docs.md`). |
| `specs/nist/*` | mark implemented, refresh baseline + census |
| `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` | `z` bump + entry |

**Not touched:** the lexer, the AST, every other crate.

## 3. Data / model changes

**None** — and that is a deliberate constraint, not an accident.

`IdentificationDivision` (`cobolt-ast/src/program.rs:19`) **already declares all
six fields**, including `installation`, `date_compiled` and `security`. They are
simply never filled in. So R7 is satisfied by assigning them, with no struct
change.

`REMARKS` has no field and **will not get one**. It is not a COBOL-85 paragraph
at all — it was deleted from the standard in 1985 — and **CCVS85 does not
contain a single `REMARKS` paragraph** (measured: 0; see §9). It will be
*accepted and discarded*, which keeps the AST untouched.

That matters more than it looks: `Program` derives `Serialize`/`Deserialize` and
is bincode-serialized into compiled binaries by `cobolt-compiler`. Adding a
field would change that format. Avoiding the change avoids the question.

## 4. Key decisions & alternatives

- **Decision: one line-start rule, not the spec's two.** *Why:* the spec's R3
  (Area A, strict format) and R4 (keyword heuristic, free format) assume the
  parser knows the source format. It does not — `expand_copybooks` flattens
  fixed to free and the CLI then tokenizes as `SourceFormat::Free`. A rule keyed
  on `span.line` changing works identically under both. *Rejected:* threading
  the format down into the parser — a new parameter on a hot path, to support a
  distinction the standard does not actually need here.
- **Decision: `INSTALLATION` / `SECURITY` / `REMARKS` stay `Identifier`s,
  matched by string inside the ID division only.** *Why:* making them keywords
  would reserve them **program-wide**, so an existing user data item named
  `SECURITY` would stop compiling. That is a silent regression for a real word
  people use. *Rejected:* adding `Token::Installation` etc. to `keywords.rs`.
- **Decision: the "followed by" guards on `DATA` / `PROCEDURE` / `ENVIRONMENT`
  and on the named identifiers.** *Why:* CM101M proves prose contains these
  words. Line-start alone would still break on a comment-entry line that happens
  to begin with `DATA`. *Rejected:* line-start alone.
- **Decision: keep the entry text, do not interpret it.** `DATE-COMPILED` is
  replaced with the compile date by some vendors; COBOL-85 permits but does not
  require it, NIST does not test it, and rewriting it would destroy what the
  developer wrote (GOLDEN RULE — user code is sacred). *Rejected:* substitution.
- **Decision: no obsolete-feature flagging.** NC303M's comment says
  "Message expected for above statement: OBSOLETE", but flagging is its own
  conformance level and is declared out of scope in
  `NIST-spec-out-of-scope-modules.md`. Adding warnings for constructs we support
  would be noise for existing users.

## 5. Risks & mitigations

| Risk | Mitigation |
|---|---|
| **A comment-entry swallows the ENVIRONMENT DIVISION** and the program silently loses its configuration. This is the failure mode the current code already has, mirrored. | The `Division`-follows guard, plus a test that a program whose `SECURITY` text ends with the word `DATA` still finds its DATA DIVISION. |
| **A generated form `.cbl` regresses.** The RAD generator emits `AUTHOR` in its banner, so this code runs on every generated program. | A test that compiles a generated-form source; `cargo test -p cobolt-forms --features render` stays green. Highest-value guard here. |
| **A user data item named `SECURITY` or `INSTALLATION` breaks.** | Avoided by design (§4) — they never become keywords. Add a test declaring `01 SECURITY PIC X.` and using it. |
| **An unterminated comment-entry runs to EOF** when a program has no ENVIRONMENT/DATA/PROCEDURE division. | `Eof` is in the terminator set; a program that is only an ID division must still parse. Test it. |
| **`REMARKS` text is discarded silently.** | Intended (§3), but say so in the docs so nobody reports it as data loss. |
| **The 32-program bucket does not fully clear**, because CM programs are out of scope anyway. | Expected: CM101M–CM105M are N/A. The *in-scope* gain is smaller than 32 — see §6's honest expectation. |

## 6. Test strategy

**Parser tests** (`cobolt-parser`), asserting the stored text, not just "no error":

| Case | Asserts |
|---|---|
| CM101M's nine-line `INSTALLATION` (verbatim) | parses; `installation` contains `GENERAL SERVICES` **and** `FALLS CHURCH`, proving it did not stop at the first period |
| `SECURITY.` / `AUTOMATED DATA AND TELECOMMUNICATION SERVICE.` | parses; the DATA DIVISION that follows is still found |
| EXEC85's two-line quoted `INSTALLATION` | both lines captured |
| `DATE-COMPILED.  22ND AUG 1988.` | parses; `date_compiled` is `Some` |
| All six paragraphs, in a scrambled order | each lands in its own field (R5) |
| No optional paragraphs at all | unchanged (regression) |
| `01 SECURITY PIC X.` in WORKING-STORAGE, then `MOVE "A" TO SECURITY` | still compiles — the word is not reserved |
| A program with only an ID division | `Eof` terminates the entry |
| A generated form `.cbl` | compiles as before |

**Suite-level** — the verdict:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

**Honest expectation.** The 32-program bucket is dominated by **CM**, which is
out of scope (9 programs) — those will start parsing but stay N/A. The
`DATE-COMPILED` 6 are in scope. Expect the in-scope PASS to rise from
**237 / 434** by roughly **8–15**, not 38. Record what actually happens; do not
claim the bucket size as the gain.

**Regression** — serially, reading every `test result:` line:

```bash
cargo test -p cobolt-lexer -p cobolt-parser -p cobolt-semantic --no-fail-fast
cargo test -p cobolt-runtime --no-fail-fast
cargo test -p cobolt-forms --features render --no-fail-fast
```

Concurrent `cargo test` jobs deadlock here (nested `cargo build` inside
`cobolt-compiler` / `cobolt-runtime` tests contends for the `target/` lock).

## 7. Steering compliance

- [x] **i18n** — no new UI strings; parser diagnostics are compiler output, not
      `Tr` fields.
- [x] **Generated-code contract** — the RAD banner emits `AUTHOR`, so this code
      is on the generated path. Pinned by a test; the forms suite must stay green.
- [x] **Docs** — English canonical only (`cobol85-supported-syntax-en.md`: the
      six paragraphs, and the scoreboard). No translation of that file exists,
      so GOLDEN RULE #8's deletion step is moot. The Developer's Guide needs
      nothing: a PowerCOBOL/isCOBOL developer already expects `AUTHOR` to work.
- [x] **System KB** — no control, property, method or event change;
      `chunked.data` untouched. Confirm the freshness test during `/implement`.
- [x] **Fix vs feature** — **fix** (missing COBOL-85 support = technical debt).
      Bump `z`, CHANGELOG entry, forum **f=97** when merged to `main`.
- [x] **User code is sacred** — the entry text is stored verbatim, never
      rewritten.
- [x] **Branch** — `fixes` (current).

## 8. Open questions

Both spec questions resolve here:

- **Q1** (should a comment-entry be allowed in free format?) — **yes**, and the
  §1 rule handles it without a format-specific heuristic, so the spec's R4 is
  superseded rather than implemented.
- **Q2** (replace `DATE-COMPILED` with the compile date?) — **no.** Store the
  source text verbatim; see §4.

One new question for `/tasks`:

- **Q3** — what to do about `REMARKS`, which **NIST does not exercise at all**
  (§9)? Three readings of the 2026-08-25 ruling ("follow NIST; where there is no
  recommendation, raise an error"):
  1. **Drop it from scope** — NIST is the source of truth and is silent, so
     `REMARKS.` stays whatever it is today.
  2. **Accept and discard** — a courtesy to developers migrating COBOL-74
     source, where `REMARKS` was valid. Costs one line in the match.
  3. **Error** — the strict reading: not in COBOL-85, so reject it.

  Recommendation: **(2)**. It is free, it cannot break anything (the word is not
  reserved either way), and rejecting a paragraph that every pre-1985 compiler
  accepted would be gratuitous for the migrating PowerCOBOL/isCOBOL audience the
  Developer's Guide is written for. But this is a scope call and NIST does not
  make it for us — **operator decision.**

## 9. Census correction — `REMARKS`

The spec's blast-radius line says "1 uses `REMARKS`". That is **wrong**, and the
error is the same one that inflated the `COPY` count in
`NIST-spec-harness-and-baseline.md`: the census matched the substring inside a
string literal, `02 FILLER PIC X(7) VALUE "REMARKS".`

Re-measured over non-comment source, matching only a paragraph header at the
start of a line:

| Paragraph | Real headers in CCVS85 |
|---|---:|
| `INSTALLATION.` | 34 |
| `SECURITY.` | 34 |
| `DATE-COMPILED` | 3 |
| `REMARKS.` | **0** |

`AUTHOR` and `DATE-WRITTEN` already work, so the in-scope work is the first
three rows. Treat any census figure produced by a substring match as an upper
bound until it has been re-measured this way.
