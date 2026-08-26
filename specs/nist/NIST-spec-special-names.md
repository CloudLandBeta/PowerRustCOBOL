# NIST-spec — the SPECIAL-NAMES paragraph

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** 19 programs use `ALPHABET`, 2 use `CURRENCY SIGN`, 1 uses
  `DECIMAL-POINT IS COMMA`. Mnemonic-name usage is broader: `DISPLAY … UPON
  DISPLAY-OUTPUT-DEVICE` appears 21 times, and switch-status condition names
  appear across the NC module.

## 1. Overview

`SPECIAL-NAMES` is where a COBOL-85 program binds implementor-names to
mnemonic-names, declares switch-status condition-names, defines alphabets and
classes, and changes the currency sign and decimal point.

PowerRustCOBOL parses the paragraph by **skipping it**. The CONFIGURATION
SECTION loop in `crates/cobolt-parser/src/parser.rs:476` consumes tokens until
it reaches `INPUT-OUTPUT`, `DATA`, `PROCEDURE`, `IDENTIFICATION` or EOF,
capturing only `DECIMAL-POINT IS COMMA` (and the PowerRustCOBOL-specific
`REPOSITORY` / `EXEC RUST` items).

That is why the paragraph does not *error* — measured: a probe containing
switch-status names, `ALPHABET TEST-ALPHABET IS NATIVE` and `CURRENCY "<"`
parses clean — while none of it takes effect. NIST tests the effects.

CCVS85 usage:

```cobol
SPECIAL-NAMES.
    XXXXX051
    IS ABBREV-SWITCH
        ON  ON-SWITCH
        OFF IS  OFF-SWITCH
    ALPHABET TEST-ALPHABET IS NATIVE
    CURRENCY  "<".
```

and elsewhere `ALPHABET MY-FAVORITE-ALPHABET IS STANDARD-1`,
`ALPHABET TAPE-CHARACTER-SET IS STANDARD-1`,
`ALPHABET AMERICAN-INDIAN IS NATIVE`.

## 2. Goals / Non-goals

- **Goals:** parse the whole paragraph into the AST and make the clauses NIST
  exercises actually work.
- **Non-goals:**
  - Removing anything. `DECIMAL-POINT IS COMMA` and the `REPOSITORY` /
    `EXEC RUST` handling stay exactly as they are; this spec only stops the rest
    of the paragraph being discarded.
  - Code-set translation on I/O (`CODE-SET` on an FD) beyond what NIST checks.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall parse `implementor-name IS
  mnemonic-name` and make the mnemonic usable in `DISPLAY … UPON`,
  `ACCEPT … FROM` and `WRITE … ADVANCING mnemonic`.
- **R2 (ubiquitous):** The system shall parse switch-status clauses —
  `implementor-name [IS mnemonic] ON [STATUS] IS cond-1 OFF [STATUS] IS cond-2`
  — in either order, with `ON`/`OFF` optional-word forms, and shall make the
  condition-names testable in `IF`.
- **R3 (ubiquitous):** The system shall parse `ALPHABET alphabet-name IS
  {STANDARD-1 | STANDARD-2 | NATIVE | implementor-name | literal-list}`, where a
  literal list may use `THROUGH`/`THRU` and `ALSO`.
- **R4 (ubiquitous):** A declared alphabet shall be usable in `PROGRAM
  COLLATING SEQUENCE`, in `SORT … COLLATING SEQUENCE`, and in an FD `CODE-SET`.
- **R5 (ubiquitous):** The system shall parse `CLASS class-name IS literal-list`
  and make the class usable as a class condition, including as an `EVALUATE`
  subject (`NIST-spec-statement-grammar-gaps.md` R5).
- **R6 (ubiquitous):** The system shall parse `CURRENCY [SIGN] IS literal` and
  use that character as the currency symbol in numeric-edited PICTURE strings
  handled by `crates/cobolt-runtime/src/numedit.rs`.
- **R7 (ubiquitous):** The system shall keep `DECIMAL-POINT IS COMMA` working
  exactly as today, including its existing diagnostic.
- **R8 (ubiquitous):** The system shall accept the clauses in any order and any
  subset, separated by optional separator commas and semicolons
  (`NIST-spec-separators.md`).
- **R9 (constraint):** The system shall not reject an implementor-name it does
  not recognise. CCVS85 uses `XXXXX051`, `XXXXX055`, `XXXXX082`, `XXXXX083` as
  placeholders; an unknown implementor-name shall bind the mnemonic and, if it
  is used at run time, produce a clear diagnostic then — not at parse time.

## 4. Acceptance criteria

- [ ] AC1 — NC108M's SPECIAL-NAMES paragraph parses into an AST with one
      mnemonic, two switch-status condition-names, one alphabet and a currency
      sign of `<`.
- [ ] AC2 — `DISPLAY X UPON DISPLAY-OUTPUT-DEVICE` routes to the bound device.
- [ ] AC3 — `IF ON-SWITCH` and `IF OFF-SWITCH` evaluate against the switch.
- [ ] AC4 — `ALPHABET A IS NATIVE` used as `PROGRAM COLLATING SEQUENCE` changes
      the result of a `SORT` in a way the program can observe.
- [ ] AC5 — `CURRENCY "<"` makes `PIC <9(5).99` edit with `<`.
- [ ] AC6 — `CLASS DIGITS IS "0" THRU "9"` works as `IF X IS DIGITS`.
- [ ] AC7 — `DECIMAL-POINT IS COMMA` behaviour and its diagnostic are unchanged
      (regression test).
- [ ] AC8 — A `REPOSITORY` paragraph and a CONFIGURATION-SECTION `EXEC RUST`
      block still parse and bind exactly as today (regression test — spec 005
      and spec 041 depend on this code path).

## 5. Constraints & steering check

- **i18n:** `CURRENCY SIGN` and `DECIMAL-POINT` are the two clauses a localized
  application uses; the Developer's Guide note should be written with that in
  mind.
- **Generated-code contract:** **high risk.** The CONFIGURATION SECTION loop
  being changed is the same one that captures `REPOSITORY` and item-level
  `EXEC RUST` (spec 041 R19). AC8 exists specifically to pin that.
- **Docs:** `docs/cobol85-supported-syntax-en.md` does not currently list
  SPECIAL-NAMES clauses at all; add them.
- **Fix vs feature:** **fix**.

## 6. Open questions

- Q1: What should an implementor-name bind to in practice? Recommendation: a
  small table — `SYSOUT`/`CONSOLE` → standard output, `SYSIN` → standard input,
  printer names → the assigned file — plus R9's late diagnostic for anything
  else. The CCVS85 `XXXXXnnn` substitution table (baseline spec Q1) feeds this.
- Q2: `ALPHABET … IS STANDARD-1` is ASCII, `STANDARD-2` is ISO 646. Does the
  runtime need real translation, or is identity acceptable on a
  native-ASCII host? NIST's alphabet tests compare collation results, so at
  minimum collation must change; translation on `CODE-SET` may be deferrable.
