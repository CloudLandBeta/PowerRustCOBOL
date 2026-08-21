<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-concise
description: >-
  Write concise, non-verbose RustCOBOL. RustCOBOL evaluates full numeric/string
  EXPRESSIONS as the source of MOVE / SET / COMPUTE and lets a computed value be
  assigned straight to a control `::` property — so you almost never need a scratch
  WORKING-STORAGE item or a multi-step MOVE→COMPUTE→MOVE chain. Load and apply this
  whenever you generate a handler or procedure: prefer the one-statement form.
---

# Writing concise RustCOBOL (agent skill)

RustCOBOL supports **expression evaluation everywhere a sending value is
expected** and **direct assignment to control properties**. The runtime evaluates
the source of `MOVE` / `SET` / `COMPUTE` with the full expression evaluator
(arithmetic, parentheses, `::` reads, reference modification). Because of this, an
intermediate `PIC` item to "hold" a number is usually **dead weight** — compute
inline. Your default is the **shortest correct statement**.

## 1. The core rule — collapse temp-var chains into one statement

Do **not** write this (verbose: a scratch item + copy + compute + copy):

```cobol
       WORKING-STORAGE SECTION.
       01  TEMP-VALUE  PIC S9(9) COMP-5.
       PROCEDURE DIVISION.
       MAIN SECTION.
           MOVE Slider-1::Value TO TEMP-VALUE
           COMPUTE TEMP-VALUE = TEMP-VALUE * 10
           MOVE TEMP-VALUE TO TextBox-1::Value.
```

Write this — one statement, no scratch item:

```cobol
       PROCEDURE DIVISION.
       MAIN SECTION.
           SET TextBox-1::Value TO Slider-1::Value * 10.
```

All three of these are equivalent and idiomatic — the arithmetic is evaluated and
the result assigned straight to the property:

```cobol
           SET     TextBox-1::Value TO Slider-1::Value * 10.
           COMPUTE TextBox-1::Value =  Slider-1::Value * 10.
           MOVE    Slider-1::Value * 10 TO TextBox-1::Value.   *> expression source
```

- `SET target TO <expression>` and `MOVE <expression> TO target` both **evaluate
  the source expression** (RustCOBOL accepts an expression, not just a single item,
  as the sending field) and store it. `COMPUTE target = <expression>` is the
  classic form and equally fine.
- The target may be a control property lvalue (`Ctrl::Value`, no parens) or a data
  item — assign to whichever the task needs, no scratch step in between.

## 2. `::` property reads are first-class values — no PIC needed

A `Ctrl::Property` read (no parens) is a value you can drop directly into any
expression, condition, or sending position. There is **no need** to `MOVE` it to a
`PIC` item first ("type inference" — the numeric/text type flows through the
expression):

```cobol
           IF Slider-1::Value > 50
               MOVE "#FF0000" TO Label-1::ForegroundColor
           END-IF
           COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value
           SET   OK-Btn::Enabled TO Name-Box::Text NOT = SPACES
           DISPLAY "score=" Score-Box::Value 
```

Only introduce a `WORKING-STORAGE` item when you genuinely need it (see §5).

## 3. Parentheses and precedence

Use parentheses for grouping/clarity; RustCOBOL honours standard arithmetic
precedence (`*` `/` before `+` `-`, left-to-right, `**` for power):

```cobol
           SET     Bar-1::Value TO (Done-Box::Value / Total-Box::Value) * 100 
           COMPUTE Area-Lbl::Value = 3.14159 * (R-Box::Value ** 2) END-COMPUTE
```

## 4. Reference modification — concise substrings of STRING values

`ref(start:length)` takes `length` characters starting at 1-based `start`;
`ref(start:)` runs to the end. Use it to slice **alphanumeric** data or **text**
properties without a work field. (It is for character data — do not apply it to a
numeric result to "convert" it.)

```cobol
           MOVE Name-Box::Text(1:3)  TO Area-Code.        *> first 3 chars
           MOVE Path-Box::Text(5:)   TO WS-TAIL.          *> char 5 → end
           IF   Code-Box::Text(1:1) = "A"
               CONTINUE
           END-IF
```

## 4b. Comments — ALWAYS `*>` (never a bare `*`)

Whenever you add a comment, use **`*>`** followed by a single space, then the text.
Never a fixed-format `*`.

- **Indent the comment to line up with the code it describes** — the `*>` goes at
  the same column as the statement it comments. Do NOT force it to column 7.
- **Wrap at column 80:** if the text would pass column 80, break the line, keep the
  **same indentation**, and start the continuation with `*>` and a space — repeat
  until the comment ends.
- Inline comments (after code) also use `*>`.

```cobol
           *> Este é um comentário que ultrapassaria a coluna 80, então o texto
           *> continua na próxima linha com o mesmo recuo e um novo *> até
           *> terminar o comentário.
           IF Name-Box::Text = SPACES         *> nada foi digitado
               CONTINUE
           END-IF
```

Never write `      * text` (a lone `*` comment); the `*` must be immediately followed
by `>`.

## 5. When a WORKING-STORAGE item IS still the right call

Be concise, not reckless. Declare an item (typed per the `rustcobol-types` skill)
when you truly need one:
- an **accumulator/counter** whose value persists across statements or a loop;
- a **table** (`OCCURS`) or a structured record;
- a **LINKAGE** item the event delivers;
- a value you must **round/format** through a numeric-edited picture for display.

Otherwise, prefer the inline expression.

## 6. Anti-patterns to avoid (rewrite them)

- A scratch `PIC` item used **once** to shuttle a value → inline the expression.
- `MOVE a TO T` then `COMPUTE T = T <op> b` then `MOVE T TO dst` → one
  `SET dst TO a <op> b` / `COMPUTE dst = a <op> b`.
- Reading a property into a temp just to compare it → compare the `::` read
  directly in the `IF`.
- Repeating the same sub-expression → factor it with parentheses or a single
  local item if reused many times.

## 7. Scope delimiters in PROCEDURE DIVISION
Implicit Scope DelimiterPeriod (.) closes the current statement and every other open/unterminated nested statement preceding it. Using explicit END-XXX terminators is preferred in modern COBOL standards to avoid unintended scope closures.

Explicit Scope Terminators in COBOL:
END-ACCEPT
END-ADD
END-CALL
END-COMPUTE
END-DELETE
END-DISPLAY
END-DIVIDE
END-EVALUATE
END-IF
END-INVOKE
END-JSON
END-MULTIPLY
END-PERFORM
END-READ
END-RETURN
END-REWRITE
END-SEARCH
END-START
END-STRING
END-SUBTRACT
END-UNSTRING
END-XML
END-WRITE

- Periods (".") are required only at:
  1. Before a SECTION 
     ```cobol
     IF a <op> b 
        MOVE x to y
     END-IF.*> This period is required

  SHOW-MORTGAGE SECTION.
     DISPLAY x.

  2. Before a PARAGRAPH
  ```cobol
     MOVE 1 TO x. *> This period is required

  EXPORT-MORTGAGE. *>  Paragraph
     WRITE RECORD-MORTGAGE INVALID KEY DISPLAY END-WRITE.
  ```


  2. Never use periods inside the scope of a COBOL verb:
     ```cobol
     PERFORM 10 TIMES
        IF a <op> b 
            MOVE x to y. *> This period is not allowed here: it ends the PERFORM block early, so the next line is not included in the loop.
     END-PERFORM
    ```
    For cases as the above use the defined COBOL-85 standard verb scope delimiter (END-IF, END-COMPUTE, END-READ, END-WRITE etc)
     ```cobol
     PERFORM 10 TIMES
        IF a <op> b 
            MOVE x to y
        END-IF
     END-PERFORM
    ```
- 


## Checklist before you emit a handler body
1. Did I declare a `WORKING-STORAGE` item that is used exactly once as a
   pass-through? → delete it and inline the expression.
2. Can a `MOVE→COMPUTE→MOVE` chain become a single `SET`/`COMPUTE` to the target
   property? → do it.
3. Am I reading a `::` property into a temp only to test/use it once? → use the
   read inline.
4. Keep the result **correct** (types per `rustcobol-types`) and **English**; concise
   never means wrong.
