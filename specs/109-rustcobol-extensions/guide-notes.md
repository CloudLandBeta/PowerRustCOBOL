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
