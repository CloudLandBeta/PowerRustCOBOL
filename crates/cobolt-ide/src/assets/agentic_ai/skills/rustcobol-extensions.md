<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-extensions
description: How RustCOBOL extends COBOL-85 — event-handler / procedure structure,
  the COBOL-2002-style `::` control method/property syntax, shared vs local state,
  and procedure reuse without low-level control CALLs.
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
- A procedure (`create_procedure`) has the **same** body shape: it is a nested
  program of its own, reached with `CALL "ITS-NAME"` and never with `PERFORM`.

## 2. Read/write properties and invoke methods with `::`

This is the main GUI extension. A control property or method is reached with the
COBOL-2002-style **`::`** member operator, using the exact property/method name
from the properties pane and method context
(**case-insensitive**): `Caption`, `Text`, `BackgroundColor`, `ForegroundColor`,
`Value`, `Visible`, `Enabled`, `Checked`, `RefreshBinding`, `SetFocus`, …

**Never use the `CALL` verb for controls or control-backed runtime services.**
Do not emit `CALL "COBOL-SET-PROPERTY"`, `CALL "COBOL-GET-PROPERTY"`,
`CALL "COBOL-CHART-..."`, or any other `CALL` as a substitute for a control
property or method. Do not use legacy `INVOKE Control "Method" USING ...`
syntax either. Use:

```text
<control>::<property>
<control>::<method>(<parameters>)
```

**Read (GET)** — `control-id::Property` is a value usable anywhere:

```cobol
           DISPLAY Button-1::Caption.
           MOVE   TextBox-1::Text TO WS-NAME.
           IF     TextBox-1::Text = SPACES
               DISPLAY "empty".
           MOVE   Button-1::"Caption" TO WS-NAME.      *> quoted name = identical
```

**Write (SET)** — assign to `control-id::Property`:

```cobol
           MOVE "Hello!" TO Button-1::Caption.
           SET  Button-1::"Caption" TO "Hello!".
```

**Invoke a method** — call the method inline with parentheses:

```cobol
           TextBox-1::SetFocus().
           DataGrid-1::RefreshBinding().
           LineChart-1::Clear().
           LineChart-1::AddPoint("Jan", WS-TOTAL).
```

- **Numeric** properties are algebraic: `IF Slider-1::Value > 50`,
  `MOVE WS-N TO Spinner-1::Value` — no intermediate `PIC` item needed.
- Colours are `#RRGGBB` string literals: `MOVE "#008000" TO Label-1::ForegroundColor`.
- **Repeating groups (arrays):** address the firing item with its index —
  `Name(CONTROL-ARRAY-INDEX)::Property`, e.g.
  `MOVE "#FFCC00" TO Row-Label(CONTROL-ARRAY-INDEX)::BackgroundColor`.

If a requested method/property is not listed in the CONTEXT, do not invent it.
Ask the user for directions or leave a `*>` comment explaining the missing member.

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

A common procedure is a nested PROGRAM, not a paragraph of your handler, so
`CALL` is the only verb that reaches it — `PERFORM VALIDATE-INPUT` has no target
and the body is rejected. Keep `PERFORM` for paragraphs you declare inside the
body you are writing, and end the main flow with `GOBACK.` before the first of
them so control does not fall through into it.

Procedure names are UPPER-CASE with hyphens (`VALIDATE-INPUT`, `RECALC-TOTAL`).

## 5. Event data (LINKAGE)

When an event delivers data, its items appear in the `LINKAGE SECTION` and are
bound by `PROCEDURE DIVISION USING …`. Use **only** the LINKAGE items the CONTEXT
lists for that event. Most events currently deliver **no** data → an empty
`LINKAGE SECTION` and a plain `PROCEDURE DIVISION.` (no `USING`). Array handlers
receive `CONTROL-ARRAY-INDEX PIC S9(4) COMP-5` (the 1-based firing index).

## 6. Advanced controls and services

Use the control object methods/properties for data, network, and chart controls.
Do not write low-level runtime `CALL`s for these services.

- **SQL** (SqlDatabase): use methods on the SqlDatabase control.
- **HTTP** (RestClient): use methods on the RestClient control.
- **Charts:** use methods such as `Chart-1::Clear()`, `Chart-1::AddPoint(...)`,
  `Chart-1::Refresh()`, or properties such as `Chart-1::DataSource`.
- **IndexedFile:** use methods such as `File-1::Open("I-O")`,
  `File-1::ReadNext()`, `File-1::Write()`, and `File-1::Close()`.

Do not invent method names. If unsure of the exact method/property, keep the
handler simple and ask for directions.

## 7. Never write

- The generated program wrapper, the event loop, form initialization, or any
  other control's working-storage.
- `CALL` for control methods, property get/set, chart actions, data bindings,
  REST actions, SQL actions, or IndexedFile actions.
- `IDENTIFICATION DIVISION` / `PROGRAM-ID` / `GOBACK` / `END PROGRAM` in a handler
  or procedure body.
- Non-English identifiers, comments, or literals-as-identifiers.
