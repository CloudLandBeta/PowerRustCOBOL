---
name: powerrustcobol-write-common-procedure
description: "Write reusable COBOL in Common Code (src/) and call it from form handlers, instead of repeating logic in several handlers."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-write-common-procedure

Write reusable COBOL in Common Code (src/) and call it from form handlers, instead of repeating logic in several handlers.

## Steps

1. Write `src/<name>.cbl` as an ordinary COBOL-85 program: `IDENTIFICATION DIVISION.`, `PROGRAM-ID. <NAME>.`, its data, a `LINKAGE SECTION` for what the caller passes, and `PROCEDURE DIVISION USING …` ending in `GOBACK`.
2. Check every verb and clause against `docs/powerrustcobol/cobol85-supported-syntax.md`; write built-ins inline (`COBOL::"NAME" ( … )`).
3. Call `add_to_project` with the path, then `check` with it until it reports no error.
4. From a handler, `CALL "<NAME>" USING …` with the arguments in the order of the procedure's `USING`; then `check` the form.

## Tools

`add_to_project` (`mcp__powerrustcobol-ide__add_to_project`, or `mcp__powerrustcobol__add_to_project` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

## Example

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALC-TAX.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-AMOUNT   PIC 9(7)V99.
       01 LK-TAX      PIC 9(7)V99.
       PROCEDURE DIVISION USING LK-AMOUNT LK-TAX.
           COMPUTE LK-TAX ROUNDED = LK-AMOUNT * 0.2
           GOBACK.
```

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.
