<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-extensions
description: How RustCOBOL extends COBOL-85 — event-handler / procedure structure,
  the `::` control-property syntax, shared vs local state, and calling procedures.
  Load this whenever generating COBOL for a handler or procedure.
---

# RustCOBOL extensions to COBOL-85 (agent skill)

You are generating COBOL for **RustCOBOL** — COBOL-85 **plus** PowerRustCOBOL's
extensions. Standard COBOL-85 you already know; this skill covers only what is
**different**, so the code you emit runs on this runtime. When a request needs a
handler or a procedure, follow this exactly.

## 1. A handler / procedure is a nested program body — and you write only the body

Every event handler and every common procedure is a **nested COBOL-85 program**.
The IDE supplies the `IDENTIFICATION DIVISION` / `PROGRAM-ID` header and the
closing `GOBACK` / `END PROGRAM` automatically. **You must NOT write those.**

Your `code` starts at `ENVIRONMENT DIVISION` and ends at your last statement.
Return a complete body every time; never return only `PROCEDURE DIVISION` or only
the statements you changed.

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.
```

Rules:
- **Do NOT** emit `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or
  `END PROGRAM` — the IDE adds them. Emitting them breaks generation.
- **Always include `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, and
  `PROCEDURE DIVISION.` in the body.** If the handler has no local data, keep the
  `DATA DIVISION.` and go straight to `PROCEDURE DIVISION.`. If the existing
  handler has `WORKING-STORAGE SECTION.` or `LINKAGE SECTION.`, preserve every
  declaration still referenced by the procedure.
- Keep fixed-format-safe indentation: divisions/sections at column 8 (area A),
  statements at column 12 (area B).
- **Comments — ALWAYS use `*>` (never a bare `*`).** Mandatory. Write `*>`, then a
  single space, then the text.
  - **Indent the comment to the code it describes.** The `*>` sits at the **same
    column as the statement** it comments — do NOT force it to column 7. A comment
    placed above a statement uses that statement's indentation.
  - **Wrap at column 80.** If a comment line would pass column 80, break it and
    continue on the next line at the **same indentation**, starting again with `*>`
    and a space, until the comment ends.
  - Inline comments (after code) also use `*>`.
  - **Never** emit a fixed-format `*` comment (`      * text`), and never place the
    `*` anywhere except immediately before `>`.
  - Example — aligned to the code, wrapped past column 80:
    ```cobol
           *> Validate every field before saving because a blank key would
           *> corrupt the index; abort the handler on the first failure.
           IF Name-Box::Text = SPACES
               GOBACK.
           MOVE 0 TO WS-N.   *> reset the counter (inline comment)
    ```
- Include only the sections you use (an empty `WORKING-STORAGE` / `LINKAGE` may be
  dropped), but always end with a real `PROCEDURE DIVISION`.
- A procedure (`create_procedure`) has the **same** shape; it is `CALL`-able by its
  name.

## 2. Read and write control properties with `::`

This is the main GUI extension. A control's property is reached with the **`::`**
member operator, using the exact property name from the properties pane
(**case-insensitive**): `Caption`, `Text`, `BackgroundColor`, `ForegroundColor`,
`Value`, `Visible`, `Enabled`, `Checked`, …

**Read (GET)** — `control-id::Property` is a value usable anywhere:

```cobol
           DISPLAY Button-1::Caption.
           MOVE   TextBox-1::Text TO WS-NAME.
           IF     TextBox-1::Text = SPACES
               DISPLAY "empty".
           MOVE   Button-1::"Caption" TO WS-NAME.      *> quoted name = identical
           INVOKE Button-1 "Caption" RETURNING WS-NAME. *> INVOKE form
```

**Write (SET)** — assign to `control-id::Property`:

```cobol
           MOVE "Hello!" TO Button-1::Caption.
           SET  Button-1::"Caption" TO "Hello!".
           INVOKE Button-1 "Caption" USING "Hello!".    *> INVOKE form (USING = set)
```

- **Numeric** properties are algebraic: `IF Slider-1::Value > 50`,
  `MOVE WS-N TO Spinner-1::Value` — no intermediate `PIC` item needed.
- Colours are `#RRGGBB` string literals: `MOVE "#008000" TO Label-1::ForegroundColor`.
- **Repeating groups (arrays):** address the firing item with its index —
  `Name(CONTROL-ARRAY-INDEX)::Property`, e.g.
  `MOVE "#FFCC00" TO Row-Label(CONTROL-ARRAY-INDEX)::BackgroundColor`.

Prefer `::` over the low-level `CALL "COBOL-SET-PROPERTY"` / `"COBOL-GET-PROPERTY"`
runtime primitives (those exist but `::` is the idiomatic form).

## 3. State: shared vs local

- **Local scratch** → the handler's own `WORKING-STORAGE SECTION`.
- **Shared state across handlers** → the form's global working-storage (declared
  `GLOBAL` in the outer program; visible to every handler). Do not redeclare it
  locally; just reference the names the form already defines (given in context).

## 4. Calling a common procedure

Factor shared logic into a `create_procedure` and `CALL` it by name from handlers:

```cobol
           CALL "VALIDATE-INPUT".
           CALL "RECALC-TOTAL" USING WS-QTY WS-PRICE.
```

Procedure names are UPPER-CASE with hyphens (`VALIDATE-INPUT`, `RECALC-TOTAL`).

## 5. Event data (LINKAGE)

When an event delivers data, its items appear in the `LINKAGE SECTION` and are
bound by `PROCEDURE DIVISION USING …`. Use **only** the LINKAGE items the CONTEXT
lists for that event. Most events currently deliver **no** data → an empty
`LINKAGE SECTION` and a plain `PROCEDURE DIVISION.` (no `USING`). Array handlers
receive `CONTROL-ARRAY-INDEX PIC S9(4) COMP-5` (the 1-based firing index).

## 6. Advanced runtime call families (only when the control/request needs them)

These non-standard runtime programs back the data/network controls; use them only
when the request clearly calls for that control:

- **SQL** (SqlDatabase): `CALL "COBOL-OPEN-DB"`, `"COBOL-EXEC-SQL"`,
  `"COBOL-FETCH-ROW"`/`"COBOL-NEXT-ROW"`, `"COBOL-CLOSE-DB"`.
- **HTTP** (RestClient): `CALL "COBOL-HTTP-GET"`, `"COBOL-HTTP-SET-HEADER"`,
  `"COBOL-HTTP-CLEAR-HEADERS"`.
- **Charts:** `CALL "COBOL-CHART-ADD-POINT"`, `"COBOL-CHART-CLEAR"`,
  `"COBOL-CHART-REFRESH"`, `"COBOL-CHART-SET-TABLE"`.

Do not invent runtime CALL names. If unsure of the exact argument list, keep the
handler simple and leave a `*>` comment noting what the developer must fill in.

## 7. Never write

- The generated program wrapper, the event loop (`CALL "COBOL-WAIT-EVENT"`),
  `COBOL-INIT-FORM`, or any other control's working-storage.
- `IDENTIFICATION DIVISION` / `PROGRAM-ID` / `GOBACK` / `END PROGRAM` in a handler
  or procedure body.
- Non-English identifiers, comments, or literals-as-identifiers.
