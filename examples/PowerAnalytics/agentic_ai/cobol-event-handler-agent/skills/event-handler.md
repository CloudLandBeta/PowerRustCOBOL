# COBOL Event Handler Script Agent Skill

Generate event-handler COBOL that is valid in the PowerRustCOBOL nested-program body edited by the IDE.

Expected shape:

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VALUE PIC X(80).
       PROCEDURE DIVISION.
           *> code here
```

Use inline control access:

```cobol
       SET Button-1::Caption TO "Save".
       SET Panel-1::ShadowEnabled TO 1.
       TextBox-1::SetFocus().
```

Do not use `CALL` for a control's properties or methods — those are reached with `::` only.

`CALL` is, however, how one program reaches another, and every handler is a program. The form is the OUTERMOST program of a COBOL-85 nest; each event handler and each common procedure is a separate nested program inside it:

- A common procedure (`create_procedure`) is a nested program, NOT a paragraph of your body. Invoke it with `CALL "ITS-NAME"` — `CALL "VALIDATE-INPUT".`, `CALL "RECALC-TOTAL" USING WS-QTY WS-PRICE.`. `PERFORM` can never reach it.
- `PERFORM` reaches only a paragraph or section declared in the SAME body you are writing. A `PERFORM` naming anything outside it is a compile error.
- End your main flow with `GOBACK.` before any paragraph you declare, or control falls through and runs that paragraph a second time.
