# NIST-spec — the Debug module (USE FOR DEBUGGING, DEBUG-ITEM, D lines)

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** the **DB module, 15 programs, 5 clean today**. Measured: 15
  programs use `USE FOR DEBUGGING`, 14 use `WITH DEBUGGING MODE`, and
  `DEBUG-ITEM` and its subfields are referenced 379 times.

## 1. Overview

COBOL-85's debug module has three parts, none of which PowerRustCOBOL
implements. A grep for `USE FOR DEBUGGING`, `DebugItem` or `DEBUG-ITEM` across
the lexer, parser and AST returns nothing.

**1. The `WITH DEBUGGING MODE` clause** on `SOURCE-COMPUTER` is a compile-time
switch. When present, debugging lines and debugging declaratives are compiled;
when absent, both are treated as comments.

```cobol
SOURCE-COMPUTER.
    XXXXX082
    WITH DEBUGGING MODE.
```

**2. Debugging lines** — a `D` in the indicator area. Compiled only under
`WITH DEBUGGING MODE`; comments otherwise.

**3. Debugging declaratives** —
`USE FOR DEBUGGING ON {procedure-name | ALL PROCEDURES | ALL REFERENCES OF id}`
in the DECLARATIVES section. The declarative runs before each execution of the
named procedure, and the special register `DEBUG-ITEM` describes what triggered
it. CCVS85 uses `USE FOR DEBUGGING ON ALL PROCEDURES`,
`ON ALTERABLE-PARAGRAPH`, `ON B-LEVEL-1`, `ON AT-END-PROC`.

`DEBUG-ITEM` is an implicit group with a fixed layout:

| Subfield | Picture | Contents |
|---|---|---|
| `DEBUG-LINE` | `X(6)` | source line identification |
| `DEBUG-NAME` | `X(30)` | the name that caused the trigger |
| `DEBUG-SUB-1/2/3` | `S9(4)` | subscripts, if any |
| `DEBUG-CONTENTS` | `X(n)` | the relevant data or a keyword |

## 2. A prerequisite defect: the `D` indicator is handled two different ways

`crates/cobolt-lexer/src/source.rs` contains two fixed-format preprocessors that
disagree about `D`:

| | `preprocess_fixed` (line 138) | `flatten_fixed` (line 281) |
|---|---|---|
| used by | `preprocess` | `Lexer::new`, `copybook.rs` |
| `D` in column 7 | treated as a **comment** | treated as **active source** |

The lexer path — the one that feeds the parser — compiles debugging lines
unconditionally. That is the *wrong default*: without `WITH DEBUGGING MODE`, a
`D` line must be a comment. Only 32 `D` lines exist in the distribution, so the
blast radius is small, but the inconsistency is real and is shared with
`NIST-spec-literal-continuation.md` §6, which found the same two functions
disagreeing about continuation.

## 3. Goals / Non-goals

- **Goals:** the compile-time switch, `D` lines, `USE FOR DEBUGGING`
  declaratives and the `DEBUG-ITEM` register, sufficient for the DB module.
- **Non-goals:**
  - Any change to the IDE's own debugger (`crates/cobolt-ide/src/panels/
    debugger.rs`, `DebugCmd`/`DebugEvent`). That is an unrelated PowerRustCOBOL
    feature and this spec must not disturb it.
  - Obsolete-feature flagging (see
    `NIST-spec-identification-division-comment-entries.md` §6).

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall parse `WITH DEBUGGING MODE` on the
  `SOURCE-COMPUTER` paragraph and record it as a compilation-unit switch.
- **R2 (state):** While `WITH DEBUGGING MODE` is absent, the system shall treat
  every line with `D` in the indicator area as a comment, in **both** fixed-form
  preprocessors.
- **R3 (state):** While `WITH DEBUGGING MODE` is present, the system shall
  compile `D` lines as ordinary source in Area B.
- **R4 (ubiquitous):** The system shall parse
  `USE FOR DEBUGGING ON {procedure-name | ALL PROCEDURES | ALL REFERENCES OF
  identifier}` inside DECLARATIVES.
- **R5 (state):** While `WITH DEBUGGING MODE` is present, the system shall
  execute a debugging declarative before each execution of its subject
  procedure, and on each reference to its subject identifier.
- **R6 (state):** While `WITH DEBUGGING MODE` is absent, the system shall
  compile debugging declaratives but never execute them.
- **R7 (ubiquitous):** The system shall provide the `DEBUG-ITEM` special
  register with the layout in §1 and populate it before each declarative runs.
- **R8 (constraint):** A debugging declarative shall not be triggered by
  statements executed *inside* a debugging declarative (no recursion).
- **R9 (constraint):** The system shall not alter the IDE debugger's channels or
  behaviour.

## 5. Acceptance criteria

- [ ] AC1 — Without `WITH DEBUGGING MODE`, a `D` line has no effect, under both
      `tokenize` and `preprocess`. A test pins that the two agree.
- [ ] AC2 — With `WITH DEBUGGING MODE`, the same `D` line executes.
- [ ] AC3 — `USE FOR DEBUGGING ON ALL PROCEDURES` fires once per paragraph
      execution, and `DEBUG-NAME` holds the paragraph name.
- [ ] AC4 — `USE FOR DEBUGGING ON ALL REFERENCES OF X` fires on each reference
      to X, with `DEBUG-CONTENTS` holding the value.
- [ ] AC5 — `DEBUG-SUB-1/2/3` carry subscripts for a subscripted reference.
- [ ] AC6 — R8: a `MOVE` inside a debugging declarative does not re-trigger it.
- [ ] AC7 — The DB module rises from 5 / 15, scored on each program's own
      `PASS`/`FAIL` report.
- [ ] AC8 — `cargo test -p cobolt-ide` stays green; the IDE debugger is
      untouched.

## 6. Constraints & steering check

- **i18n:** none — `DEBUG-ITEM` is a COBOL register, and per the CRITICAL
  constraint every COBOL name stays English in all six UI languages.
- **Generated-code contract:** none; the RAD generator emits no `D` lines.
- **Docs:** the Developer's Guide should distinguish clearly between COBOL's
  standard debug module and PowerRustCOBOL's IDE debugger — a
  PowerCOBOL/isCOBOL developer will otherwise assume they are the same thing.
- **Fix vs feature:** **fix** — a standard COBOL-85 module.

## 7. Open questions

- Q1: R2 changes a default: `D` lines currently execute and would stop doing so
  unless `WITH DEBUGGING MODE` is present. That is the standard's behaviour, but
  it is a behaviour change for any existing user relying on the current reading.
  **Operator ruling wanted.** Mitigation if wanted: a warning the first time a
  `D` line is skipped.
- Q2: The debug module is an obsolete element in later standards. Confirm the
  operator wants it implemented rather than declared out of scope alongside CM
  and RW (`NIST-spec-out-of-scope-modules.md`). It is 15 programs, and unlike
  CM/RW it is cheap.
