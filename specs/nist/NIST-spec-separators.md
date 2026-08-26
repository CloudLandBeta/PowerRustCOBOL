# NIST-spec — separators: comma, semicolon, and space-separated subscripts

- **Status:** draft → approved
- **Folder:** specs/nist/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-08-25
- **Blast radius:** **95 of 459 programs (20.7 %)** use a comma as an operand
  separator. 21 programs fail on it as their *first* error; 11 more fail on
  space-separated subscripts; 3 on a comma-separated `PROCEDURE DIVISION USING`
  list.

## 1. Overview

COBOL-85 defines the **separator comma** and **separator semicolon** as purely
optional punctuation. `, ` and `; ` may appear anywhere a space may appear, and
they mean nothing — a conforming compiler treats them as whitespace. Symmetric-
ally, the *only* required separator between subscripts is a space; the comma is
optional there too.

PowerRustCOBOL treats commas and semicolons as significant, so every one of
these standard forms fails:

| CCVS85 source | Program | Diagnostic |
|---|---|---|
| `MOVE ZERO TO DN3, DN4.` | IC201A | `unexpected token in statement: Comma` |
| `MOVE SPACE TO DN2, DN4.` | IC203A | ” |
| `CALL ID1 USING TABLE-01, TABLE-02, INDEX-1, DN3.` | IC207A | ” |
| `PROCEDURE DIVISION USING TABLE-1, TABLE-2, DN1.` | IC203A | `expected section name, found …` |
| `MOVE 01 TO TABLE4-NUM2 (1  1).` | NC121M | `expected RParen, found …` |
| `IF ANIMAL (1  1  1) EQUAL TO 1 …` | NC134A | ” |
| `MOVE W-3 TO TABLE-1 (INXEX1  INXEX2  INXEX3).` | NC135A | ” |
| `01  WRK-AN-X-18-1, REDEFINES WRK-XN-18-1 PIC A(18).` | NC101A | data description |
| `01  WRK-DU-X-18V0-1; REDEFINES WRK-XN-18-1 PIC 9(18).` | NC101A | data description |

The last two are worth noting: CCVS85 places a separator comma *and* a separator
semicolon between a data-name and its `REDEFINES` clause, precisely to prove
they are ignorable.

Verified in a minimal probe (`p6_stmts.cbl`):

```
L24   expected RParen, found IntegerLiteral(2)     |  MOVE 1 TO CELL (1  2).
L25   unexpected token in statement: Comma         |  MOVE ZERO TO A, B, C.
L26   expected RParen, found Identifier("IDX-B")   |  MOVE 1 TO CELL OF COLS OF ROWS (IDX-A  IDX-B).
```

The comma case also produces a **bogus cascade** into the semantic analyser —
`paragraph 'C' is declared more than once` — because the fragment after the
comma is re-read as a paragraph header.

## 2. Goals / Non-goals

- **Goals:** treat `,` and `;` as whitespace wherever COBOL-85 does, and accept
  subscript lists separated by spaces alone.
- **Non-goals:**
  - Changing the decimal comma. Under `DECIMAL-POINT IS COMMA` a comma inside a
    numeric literal is a decimal point, and that stays exactly as it is — see
    §5 and `NIST-spec-special-names.md`.
  - Making the comma *required* anywhere.

## 3. Requirements (EARS)

- **R1 (ubiquitous):** The system shall treat a separator comma (`,` followed by
  a space or end of line) as equivalent to a space.
- **R2 (ubiquitous):** The system shall treat a separator semicolon (`;`
  followed by a space or end of line) as equivalent to a space.
- **R3 (ubiquitous):** The system shall accept separator commas and semicolons
  in, at minimum: receiving-operand lists (`MOVE … TO a, b, c`), `USING`
  argument lists on `CALL` and on the `PROCEDURE DIVISION` header, `ENTRY`
  argument lists, subscript lists, `FUNCTION` argument lists, data description
  entries between clauses, `VALUE` literal lists, `OCCURS … KEY` lists, and
  `SELECT`/file-control entries.
- **R4 (ubiquitous):** The system shall accept subscripts separated by one or
  more spaces with no comma: `TABLE (1 1 1)` is identical to `TABLE (1, 1, 1)`.
- **R5 (ubiquitous):** The system shall accept a qualified, subscripted
  reference — `CELL OF COLS OF ROWS (IDX-A IDX-B)` — combining `OF`/`IN`
  qualification with a space-separated subscript list.
- **R6 (state):** While `DECIMAL-POINT IS COMMA` is in effect, the system shall
  continue to read a comma between digits as a decimal point, and shall then
  require a semicolon or space where a separator comma would otherwise be used.
- **R7 (constraint):** The system shall not emit a semantic diagnostic derived
  from mis-parsed separator punctuation. The `paragraph 'C' is declared more
  than once` cascade shall not occur.
- **R8 (constraint):** The system shall not require a comma or semicolon
  anywhere it is currently optional.

## 4. Acceptance criteria

- [ ] AC1 — `MOVE ZERO TO A, B, C.` parses and stores zero into all three.
- [ ] AC2 — `PROCEDURE DIVISION USING A, B, C.` binds three parameters.
- [ ] AC3 — `CALL "SUB" USING A, B, C.` passes three arguments.
- [ ] AC4 — `MOVE 1 TO CELL (1  2).` and `CELL (1, 2)` produce identical ASTs.
- [ ] AC5 — `CELL OF COLS OF ROWS (IDX-A  IDX-B)` resolves to the same storage
      key as the fully-parenthesised, comma-separated form.
- [ ] AC6 — `01 X, REDEFINES Y PIC A(18).` and `01 X; REDEFINES Y PIC 9(18).`
      both parse (NC101A).
- [ ] AC7 — IC201A, IC203A, IC207A, NC121M, NC123A, NC134A, NC135A no longer
      report a separator diagnostic.
- [ ] AC8 — A program with `DECIMAL-POINT IS COMMA` still reads `1,5` as one and
      a half (no regression on the existing behaviour).
- [ ] AC9 — The 21-program comma bucket and the 11-program `expected RParen`
      bucket are empty in the harness census.

## 5. Interaction with `DECIMAL-POINT IS COMMA`

These two features genuinely collide, and COBOL-85 resolves it: under
`DECIMAL-POINT IS COMMA` the *separator comma* is not available and the period
becomes the separator. PowerRustCOBOL already implements the decimal comma —
including a notably good diagnostic:

> `'12345678,91' reads as a comma decimal separator, but this compilation unit
> does not declare it…`

That behaviour must survive. R6 exists so the implementation resolves the comma
by looking at what surrounds it (digits on both sides → decimal point;
whitespace after → separator), which is exactly how the standard disambiguates.

## 6. Constraints & steering check

- **i18n:** none.
- **Generated-code contract:** the RAD generator does not emit separator commas,
  so no impact; but AC8's decimal-comma path is used by localized numeric
  handling and must not regress.
- **Docs:** `docs/cobol85-supported-syntax-en.md` gains a "separators" note.
  The Developer's Guide should say plainly that commas are decoration — a
  PowerCOBOL/isCOBOL developer will expect that and would be surprised by an
  error.
- **Fix vs feature:** **fix**.

## 7. Open questions

- Q1: Is the comma best handled in the lexer (never emit a `Comma` token when it
  is a separator) or the parser (skip it at each list site)? Recommendation:
  the lexer, because R3's list of sites is long and easy to leave incomplete —
  and the decimal-comma disambiguation already lives there.
- Q2: `Token::Comma` and `Token::Semicolon` are presumably used elsewhere in the
  grammar (`EXEC RUST`, form bindings). Those uses must be enumerated during
  `/plan` before the lexer option is chosen.
