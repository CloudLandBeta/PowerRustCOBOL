---
name: powerrustcobol-define-indexed-file
description: "Define an indexed (ISAM) file with a .cidx, generate its facade, and use the file from a form's handlers."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-define-indexed-file

Define an indexed (ISAM) file with a .cidx, generate its facade, and use the file from a form's handlers.

## Steps

1. Read `docs/powerrustcobol/cidx-format.md`. Write `indexed/<name>.cidx`: the file's COBOL name, its `assign-path` (relative paths start at the project folder, e.g. `data/customers.idx`), access mode, keys and the record's fields with their PICTUREs, offsets and lengths. Every key part names a field of the record.
2. Call `validate` with the path and fix what it reports.
3. Call `add_to_project` with the path, then `regenerate` with it: that writes `generated/<name>-indexed.cbl` and `COPYBOOKS/<name>.SEL` / `.FD`. Never edit those.
4. To use the file in a form, declare it in the form's COBOL structure: the `SELECT` in the `<file-control>` block (it may be `COPY "COPYBOOKS/<name>.SEL".`) and the `FD` in `<file-section>` as `FD <file> IS GLOBAL.` with the record exactly as the `.cidx` describes it — handlers see the record only through `GLOBAL`, and the generated `.FD` copybook is not `GLOBAL`. Give the `SELECT` a `FILE STATUS IS` item declared `GLOBAL`.
5. In handlers use the standard verbs — `OPEN`, `READ`, `WRITE`, `REWRITE`, `DELETE`, `START`, `CLOSE` — and test the file status after each. A file on disk made for another layout answers `OPEN` with status 39.
6. Make sure the data file's folder exists (`OPEN OUTPUT` creates the file, not the folder), then `regenerate` and `check` the form until it reports no error.

## Tools

`validate` (`mcp__powerrustcobol-ide__validate`, or `mcp__powerrustcobol__validate` with the IDE closed), `add_to_project` (`mcp__powerrustcobol-ide__add_to_project`, or `mcp__powerrustcobol__add_to_project` with the IDE closed), `regenerate` (`mcp__powerrustcobol-ide__regenerate`, or `mcp__powerrustcobol__regenerate` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

## Example

```cobol
       *> <file-control> block of the form
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               FILE STATUS IS WS-CUST-FS.
       *> <file-section> block of the form
       FD  CUSTOMER-FILE IS GLOBAL.
       01  CUSTOMER-RECORD.
           05 CUST-ID     PIC 9(8).
           05 CUST-NAME   PIC X(40).
```

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.
