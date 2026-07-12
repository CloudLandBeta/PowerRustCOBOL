---
name: MOVE
supported_clauses:
  - TO
  - CORRESPONDING
unsupported_clauses: []
since_version: "1.0.0"
---
# MOVE Statement

The `MOVE` statement transfers data from one area of storage to one or more other areas.

## Syntax
`MOVE {identifier-1 | literal-1} TO identifier-2 [identifier-3] ...`

## Examples
```cobol
MOVE "HELLO" TO WS-GREETING
```
