# NIST-spec — continuation lines and continued literals

- **Status:** ✅ **IMPLEMENTED 2026-08-25**, together with
  `NIST-spec-fixed-format-reference-format.md` — see that spec's §9 for what
  shipped. The two are inseparable: a continued literal runs to column 72, so it
  cannot be reassembled without the column rule.
- **Result:** the 48-program `expected PROCEDURE DIVISION` root-cause bucket
  **fell to 6**, and those 6 are the `DATE-COMPILED` / `INSTALLATION` cause that
  `NIST-spec-identification-division-comment-entries.md` owns. AC5 met.
- **R6 and AC6 closed 2026-08-25 at version 1.62.12** — see §10.
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** **396 of 459 programs (86.3 %)** contain at least one
  continued literal. 2,085 continuation lines in the distribution, of which
  **2,027 continue an alphanumeric literal**.
- **Severity:** highest of any NIST spec. This one defect produces the
  `expected PROCEDURE DIVISION` end-of-stream failure that hides every other
  diagnostic in 48 programs.

## 1. Overview

A hyphen in the indicator area (column 7) marks a **continuation line**: the
line continues the last non-blank token of the previous line rather than
starting a new one. COBOL-85 gives it two distinct meanings depending on what is
being continued.

PowerRustCOBOL's lexer does neither. `flatten_fixed`
(`crates/cobolt-lexer/src/source.rs:281`) handles the `-` indicator by replacing
it with a space and emitting the line **as its own line**:

```rust
} else if matches!(indicator, '-' | 'D') {
    out.push_str(&raw_line[..col6_byte]);
    out.push(' ');
    if char_count > 7 {
        out.push_str(&raw_line[col7_byte..line_end]);
    }
}
```

Continuation lines are therefore never joined at all. For a continued literal
the consequence is severe: the first line ends with an unbalanced quotation
mark, the lexer's string rule runs past the end of the line looking for its
closing quote, and it swallows arbitrary amounts of the program.

### Measured failure

Probe (`p5_continuation.cbl`), a 14-line program whose only unusual feature is
two continued literals:

```cobol
000700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
000800-    "------------------------------".
...
001200     MOVE
001300-             "PRESENT INCORRECT" TO RE-MARK.
```

Result: a single diagnostic, `L0 expected PROCEDURE DIVISION` — the whole
program consumed. That is the exact signature of the 48-program bucket in the
baseline census, and it is why programs like **NC108M**, **NC113M**, **SQ102A**
and **IX101A** report no line-anchored error at all: their declarations are
fine, their statements are fine, and a continued literal ate the file.

### The silent-corruption case

Where the stray quotation marks happen to balance, the program still parses —
with the **wrong literal value**. This is why
`NIST-spec-harness-and-baseline.md` R8 forbids scoring a program as passing on
a clean parse alone. A test that compares `PASS`/`FAIL` output is the only thing
that catches it.

## 2. Goals / Non-goals

- **Goals:** implement COBOL-85 continuation for alphanumeric literals, numeric
  literals and words, in the lexer path that actually feeds the parser.
- **Non-goals:**
  - Continuation in free format (COBOL-85 does not define it there).
  - Changing the `D` indicator's meaning — see
    `NIST-spec-debugging-module.md`, which owns that (and the inconsistency
    noted in §6).

## 3. The COBOL-85 rules

**Continuing an alphanumeric literal.** The continued line's literal is *not*
closed by a quotation mark; it runs to the end of Area B. The continuation line
must have a quotation mark as its first non-blank character in Area B, and the
literal resumes with the character immediately following it.

**Crucially, the continued line's content runs to column 72 — trailing spaces
included.** This is why this spec depends on
`NIST-spec-fixed-format-reference-format.md`: without cutting at column 72 the
identification area would be spliced into the literal.

CCVS85 relies on that being exact. In NC113M:

```
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------NC1134.2
011800-    "------------------------------".                            NC1134.2
```

Measured: line 011700 is exactly 80 characters, its literal fragment ends
precisely at column 72, and the two fragments are 24 + 30 = **54** characters —
matching `PICTURE X(54)`. A reassembly that is off by one character makes the
`VALUE` invalid.

**Continuing a word or a numeric literal.** Trailing blanks of the continued
line are ignored, and the first non-blank character of the continuation line
joins the previous token directly. CCVS85 uses this too — program **NC205A**
splits a data-name across lines:

```
004800-    DIVISION PICTURE X.
```

**A comment line may appear between** a continued line and its continuation; it
is not part of either.

## 4. Requirements (EARS)

- **R1 (event):** When a source line has `-` in the indicator area and the
  previous non-comment line ended inside an unterminated alphanumeric literal,
  the system shall require the first non-blank character of Area B to be a
  quotation mark, and shall resume the literal from the character after it.
- **R2 (ubiquitous):** When continuing an alphanumeric literal, the system shall
  take the continued line's contribution as its Area B content **through column
  72**, preserving trailing spaces.
- **R3 (event):** When a source line has `-` in the indicator area and the
  previous line did not end inside a literal, the system shall append the
  continuation line's first non-blank character onward directly to the last
  token of the previous line, discarding the previous line's trailing blanks.
- **R4 (ubiquitous):** The system shall apply continuation in the lexer path
  used by `tokenize` — `flatten_fixed` — not only in `preprocess_fixed`.
- **R5 (event):** When a comment line falls between a continued line and its
  continuation line, the system shall ignore the comment and still join the two.
- **R6 (constraint):** The system shall not allow an alphanumeric literal to run
  past the end of a line when no continuation line follows. An unterminated
  literal shall be reported at its own line, not silently extended.
- **R7 (ubiquitous):** Continuation shall apply equally to literals delimited by
  `"` and by `'`, and shall preserve the doubled-delimiter escape (`""` inside a
  `"` literal).
- **R8 (constraint):** The system shall not alter free-format behaviour.

## 5. Acceptance criteria

- [x] AC1 — NC113M's `HYPHEN-LINE` reassembles to exactly 54 hyphens.
- [x] AC2 — `p5_continuation.cbl` (§1) parses clean, and `RE-MARK` receives
      `PRESENT INCORRECT`.
- [x] AC3 — R6: a genuinely unterminated literal produces one diagnostic on its
      own line, and the remainder of the program still parses. The current
      whole-file swallow is gone.
- [x] AC4 — NC205A's `DIVISION PICTURE X.` continuation resolves to a single
      data-name, not to the `DIVISION` keyword.
- [x] AC5 — The CCVS85 harness `nist` pass improves from **199 / 459**; the
      48-program `expected PROCEDURE DIVISION` root-cause bucket drops to the
      programs whose cause is genuinely `DATE-COMPILED`
      (`NIST-spec-identification-division-comment-entries.md`).
- [x] AC6 — A continued literal whose stray quotes happen to balance produces
      the correct value, verified by executing the program and reading its
      output — not by a clean parse (baseline spec R8).
- [x] AC7 — Existing PowerRustCOBOL sources and generated form `.cbl` files are
      unaffected; the forms engine suite
      (`cargo test -p cobolt-forms --features render`) stays green.

## 6. Note — two continuation implementations disagree

`crates/cobolt-lexer/src/source.rs` contains **two** fixed-format preprocessors
with different continuation semantics:

| | `preprocess_fixed` (line 89) | `flatten_fixed` (line 243) |
|---|---|---|
| used by | `preprocess` | `Lexer::new`, `copybook.rs` |
| `-` continuation | appends `active.trim_start()` to the previous line | emits the line separately — no join |
| `D` indicator | treated as a comment | treated as **active source** |

The lexer — the path that feeds the parser — is the one that does not join. The
`D` divergence is a second, independent inconsistency and belongs to
`NIST-spec-debugging-module.md`. Whichever way the plan resolves this, the two
must end up agreeing, and a test should pin that they do.

## 7. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** generated form `.cbl` files do not use
  continuation lines, so no impact is expected — AC7 pins it.
- **Docs:** `docs/cobol85-supported-syntax-en.md` gains continuation under
  source format; the Developer's Guide gains a short note framed for a
  PowerCOBOL/isCOBOL developer (their compilers implement this, so their
  instinct is that it "just works" — which is exactly the trap).
- **Fix vs feature:** **fix** — a COBOL-85 construct that should already work.

## 8. Open questions

- Q1: Does joining happen in the source preprocessor (producing a single logical
  line, simplest) or in the lexer (preserving per-fragment spans, better
  diagnostics)? Recommendation: preprocessor, with a span map so a diagnostic
  inside a continued literal still points at the right physical line.
- Q2: R2 preserves trailing spaces to column 72 — but only under strict
  reference format. Under the default non-truncating format, where does a
  continued literal fragment end? Recommendation: at the end of the line as
  typed, and note in the docs that continued literals need strict reference
  format to be byte-exact.

## 10. R6 closed — measured 2026-08-25, version 1.62.12

R6 and AC6 were left open when the rest of this spec shipped at 1.62.8. Both are
now closed, and the evidence for every criterion is recorded here so the spec
does not read as done while its criteria read as untouched.

### What the defect actually was

The obvious reading — "an unterminated literal runs to end of file" — was
**wrong**, and measuring it is what corrected the design. The four programs it
blocked (SG104A, SG105A, SG106A, NC215A) each contain an **even** number of
quotation marks: 194, 194, 194 and 224. Nothing was unterminated.

One quotation mark inside prose —

```
004100     THIS PROGRAM CHECKS THE COMPILER"S ABILITY TO HANDLE EIGHT
```

— opened a literal that closed at the *next* quote several lines later,
swallowing the `ENVIRONMENT DIVISION` header between them, after which every
remaining quote in the file paired with the wrong partner. A single character
shifted the parity of an entire program.

### The fix

Two character classes in `crates/cobolt-lexer/src/token.rs`: `[^"\\]` →
`[^"\\\n]` and `[^'\\]` → `[^'\\\n]`. A literal is now a single-line construct,
which is what COBOL-85 says it is — continuation is a *source-format* mechanism
resolved by the preprocessor before the lexer runs.

The value is not merely "unterminated literals are reported": it is that quote
mis-pairing becomes **self-correcting at the next newline** instead of
cascading.

`LexError::UnterminatedString` — declared since the beginning and never
constructed — now has its first caller, and the parser names the cause instead
of reporting an unexpected character.

### Result

In-scope PASS **241 → 242 / 434**. The 6-program `expected PROCEDURE DIVISION`
bucket fell to **2** (both SM, a COPY/REPLACE cause). Of the four freed:

- **NC215A** now passes.
- **SG104A, SG105A, SG106A** advanced to `SORT-PARA SECTION 69.` — a segment
  priority number, owned by [`NIST-spec-segmentation.md`](NIST-spec-segmentation.md).
  That is why Segmentation still reads 0 / 13, and it means the module's real
  blocker is finally visible.

Third release running, the same lesson: **a bucket's size is an upper bound on
the gain, not a prediction.**

### Evidence per criterion

| AC | Evidence |
|---|---|
| AC1 | `strict_joins_a_continued_alphanumeric_literal` (parse) + `nc113m_hyphen_line_is_exactly_54_characters` (value) |
| AC2 | `a_statement_continued_mid_way_moves_the_right_text` |
| AC3 | `unterminated_literal_is_reported_locally` |
| AC4 | `strict_joins_a_continued_word` |
| AC5 | the 48 → 6 bucket drop at 1.62.8, and 6 → 2 here |
| AC6 | the three tests in `test_continuation_values.rs`, executed and asserted on `DISPLAY` output |
| AC7 | `exec_rust_body_with_a_multiline_rust_string_is_captured_whole`, `exec_rust_body_may_mention_end_exec_inside_a_string`, and the forms suite |

### Known residual

A **multi-line** Rust string inside an `EXEC RUST` block whose text contains the
characters `END-EXEC` would now terminate the block early. The block is captured
by offset slicing and the scan only matches `Word("END-EXEC")`, so this needs
all three of: a multi-line string, inside `EXEC RUST`, containing that exact
text. Not fixed, deliberately — recorded rather than left to be discovered.
