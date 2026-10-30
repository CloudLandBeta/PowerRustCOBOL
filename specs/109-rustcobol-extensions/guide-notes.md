<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Guide notes — spec 109 (kept here until 2.x.x is released; nothing under `docs/` on this line)

## S3 — Collections (2.0.104)

Audience: COBOL developers from Fujitsu PowerCOBOL and Veryant isCOBOL. To be
written into `docs/developers-guide-en.md` when 2.x.x merges to main.

**What it does.** `Split` can hand a list of pieces to the next method. Eight methods
work on a list: `Map`, `Filter`, `SortByDescending`, `Take` answer another list;
`Join`, `Sum`, `Max` answer one value. `PERFORM VARYING element THROUGH collection`
walks a list or an OCCURS table.

```cobol
       01 WS-NAMES PIC X(40) VALUE " b , a ,, c ".
           DISPLAY WS-NAMES::Split(",")
               ::Map(ITEM => ITEM::Trim())
               ::Filter(ITEM <> " ")
               ::Join("/")
      *> shows  b/a/c

           PERFORM VARYING NAME THROUGH WS-NAMES::Split(",")::Map(ITEM => ITEM::Trim())
               DISPLAY NAME
           END-PERFORM
```

**Notes**
- `ITEM => …` is an expression: a value (`Map`, the key of `SortByDescending`) or a test
  (`Filter`). It cannot assign or do I/O. The name exists only inside it.
- `Sum` of an empty list is zero; `Max` of an empty list stops the run with a message that
  names the method.
- `X::Split(sep)(n)` still answers piece n, and `X::Split(sep)` alone still answers piece 1.

**Caveats**
- A list is not a COBOL value: it cannot be `MOVE`d or `DISPLAY`ed. End a chain with `Join`,
  `Sum` or `Max`, or walk it with `THROUGH`.
- The loop element is a copy: writing to it does not change an OCCURS occurrence.
- The pieces of a `Split` of a fixed-width field keep the field's trailing blanks; `Trim` them.
- Not available in Web or Mobile projects yet: the build refuses `THROUGH` and says so.

📷 Screenshot needed: the editor's completion popup after `::` on a data item.

## S2 — Formatting (2.0.105)

**What it does.** A numeric item declared with `FORMAT IS "name"` is shown formatted
in `DISPLAY`, `STRING`, an interpolated string and `Format()`; it is stored and compared
as the number it is.

```cobol
       01 WS-LOCALE PIC X(16) VALUE "pt-BR".
       01 WS-ASSETS PIC S9(13)V99 VALUE 1234567.89
           FORMAT IS "currency BRL" LOCALE IS WS-LOCALE.
       01 WS-RATIO  PIC 9V999 VALUE 0.425.

           DISPLAY WS-ASSETS                          *> R$ 1.234.567,89
           MOVE "en-US" TO WS-LOCALE
           DISPLAY WS-ASSETS                          *> R$1,234,567.89
           DISPLAY F"Margin: {WS-RATIO:percent}"      *> Margin: 42.5%
           DISPLAY WS-RATIO::Format("percent", "pt-BR")  *> 42,5%
```

**Formats:** `number` (the item's own decimals), `currency` (the currency's decimals),
`compact` (`1,2 mi`), `currency-short` (`R$ 1,2 mi`), `percent` (the value is a ratio).
**Locales:** `pt-BR`, `en-US`, `fr-FR`. Names and tags ignore case.

**Notes**
- `{expr:format}`: the format follows a single `:`; `::` is the member operator.
  `{{` and `}}` print a brace.
- The currency is part of the format text: `"currency BRL"`. A project may name a default.
- Rounding is half away from zero; a negative value shows a leading minus.

**Caveats**
- Only a numeric item can be formatted; `MOVE` copies the stored digits, not the text.
- Not available in Web or Mobile projects yet: the build refuses a `FORMAT` clause.
- Until the project's default locale is wired in, an item with no `LOCALE` uses `en-US`.

📷 Screenshot needed: a form label showing a formatted currency in two locales.

## S1 — User-defined functions (2.0.107)

**What it does.** A `FUNCTION-ID` unit is a function you call inside an expression.

```cobol
       IDENTIFICATION DIVISION.
       FUNCTION-ID. DOUBLE-AMOUNT.
       DATA DIVISION.
       LINKAGE SECTION.
       01 ARG-AMOUNT PIC S9(5)V99.
       01 RESULT-AMOUNT PIC S9(5)V99.
       PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT RETURNING RESULT-AMOUNT.
           COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
           GOBACK.
       END FUNCTION DOUBLE-AMOUNT.

           COMPUTE WS-TOTAL = FUNCTION DOUBLE-AMOUNT(WS-PRICE) + FUNCTION DOUBLE-AMOUNT(5)
```

**Notes**
- Arguments are copies; the function never changes the caller's items.
- A function may call other functions and itself. Each call has its own arguments and result.
- Put functions you share in Common Code (`src/`): every form sees them.
- A function cannot have an intrinsic function's name (`MAX`, `SUM`, …): the build says so.

**Caveats**
- The number of arguments is checked when you build.
- Not available in Web or Mobile projects yet: the build refuses `FUNCTION` calls.

📷 Screenshot needed: the Check panel on a call with the wrong number of arguments.
