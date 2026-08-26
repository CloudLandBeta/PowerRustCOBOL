# NIST-spec — user-defined words beginning with a digit

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** small but absolute — NC203A, NC251A, NC234A, NC114M fail
  outright. These are Nucleus tests, so the NC module cannot reach 95 / 95
  without this.

## 1. Overview

COBOL-85's user-defined word is 1-30 characters drawn from `A-Z`, `0-9` and the
hyphen, and it must not begin or end with a hyphen. **It may begin with a
digit.** Two further rules matter:

- A **data-name** must contain at least one alphabetic character.
- A **paragraph-name or section-name** need not — it may be composed entirely of
  digits.

PowerRustCOBOL's lexer requires a leading letter, so a digit-leading word is
split into an integer literal and a remainder:

| CCVS85 source | Program | Diagnostic |
|---|---|---|
| `MOVE ZERO TO 25COUNT.` | NC203A | `unexpected token in statement: Count` |
| `MOVE 40 TO 25COUNT.` | NC251A | ” |
| `01  3-DEM-TBL REDEFINES 3-DIMENSION-TBL.` | NC234A | `expected identifier for REDEFINES target, found IntegerLiteral(3)` |
| `0 SECTION.` | NC114M | `unexpected token in statement: IntegerLiteral(0)` |

Note `25COUNT` splits into `IntegerLiteral(25)` plus the reserved word `COUNT`,
which is why the diagnostic names a keyword that is not in the source.

Verified in probe `p8_words.cbl`:

```
L8    expected identifier for REDEFINES target, found IntegerLiteral(3)
      | 000800 01  3-DEM-TBL REDEFINES 3-DIMENSION-TBL.
L15   unexpected token in statement: IntegerLiteral(0)
      | 001500 0 SECTION.
L15   unexpected token in statement: Section
```

## 2. Goals / Non-goals

- **Goals:** lex digit-leading user-defined words as single words, and accept
  all-numeric paragraph and section names.
- **Non-goals:**
  - Allowing a data-name with no alphabetic character. `01 25 PIC 9.` stays
    invalid — `25` there is a level number.
  - Changing level-number recognition (see §3, which is the hard part).

## 3. The disambiguation problem

A digit string at the start of a data description entry is a **level number**;
`25COUNT` in the same position is a **data-name**. The distinction is purely
lexical: a level number is digits followed by a *separator*; a digit-leading
word is digits followed immediately by a letter or hyphen.

So the rule is about what follows the digits, not about position:

> A run of digits is a numeric literal or level number only if the character
> immediately after it is not a letter, a digit, or a hyphen-followed-by-a-word
> character.

`3-DEM-TBL` is the awkward case: `3-` could be "three minus…". COBOL-85 resolves
it in favour of the word, because an arithmetic operator must be surrounded by
spaces. That rule is worth implementing generally — it also protects
`WRK-DS-18V00-S` and similar CCVS85 names.

A separate, related misclassification was observed while measuring: `.00001`
produced `LevelNumber(1)`, which shows level-number classification is currently
reachable from positions where a level number cannot occur. That is fixed by
`NIST-spec-numeric-literals.md` R6; this spec should not duplicate it, but the
two touch the same code and should be planned together.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall lex a sequence of letters, digits and
  hyphens that begins with a digit and contains at least one letter as a single
  user-defined word.
- **R2 (ubiquitous):** The system shall not treat a hyphen inside such a word as
  a subtraction operator.
- **R3 (ubiquitous):** The system shall accept a paragraph-name or section-name
  composed entirely of digits (`0 SECTION.`, `01.` as a paragraph header).
- **R4 (constraint):** The system shall continue to read a digit run at the
  start of a data description entry, followed by a separator, as a level number.
- **R5 (constraint):** The system shall continue to reject a data-name with no
  alphabetic character.
- **R6 (ubiquitous):** A digit-leading word shall be usable everywhere an
  alphabetic-leading word is — declaration, `REDEFINES` target, `MOVE`
  receiver, subscript, qualification, `PERFORM` target.

## 5. Acceptance criteria

- [ ] AC1 — `01 25COUNT PICTURE 99.` declares a data item named `25COUNT`, and
      `MOVE ZERO TO 25COUNT.` writes to it.
- [ ] AC2 — `01 3-DEM-TBL REDEFINES 3-DIMENSION-TBL.` parses; the redefines
      target resolves.
- [ ] AC3 — `0 SECTION.` declares a section named `0`, and `PERFORM 0` reaches
      it.
- [ ] AC4 — `01 X PIC 9.` still parses `01` as a level number (no regression).
- [ ] AC5 — `COMPUTE A = B - C` still parses as subtraction; `COMPUTE A = B-C`
      is a word named `B-C` per the standard's spacing rule, and whichever
      reading is chosen is documented.
- [ ] AC6 — NC203A, NC251A, NC234A and NC114M no longer report these
      diagnostics.

## 6. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator emits alphabetic-leading names
  only, so no impact — but AC4/AC5 protect every existing program.
- **Docs:** `docs/cobol85-supported-syntax-en.md` gains a "user-defined words"
  note; the Developer's Guide should mention it, since a PowerCOBOL/isCOBOL
  developer may have digit-leading names in migrated copybooks.
- **Fix vs feature:** **fix**.

## 7. Open questions

- Q1: AC5 — `B-C` with no spaces. COBOL-85 says an operator needs surrounding
  spaces, so `B-C` is a word. That is a behaviour change for anyone currently
  writing `A = B-C` and expecting subtraction. Recommendation: follow the
  standard, and emit a **warning** when a hyphenated word appears in an
  arithmetic expression and no such data item exists. **Operator ruling
  wanted** — this is the one place in these specs where conformance could
  surprise an existing user.
- Q2: Does `Token::LevelNumber` need to disappear in favour of context-sensitive
  parsing? Resolve in `/plan` alongside `NIST-spec-numeric-literals.md`.
