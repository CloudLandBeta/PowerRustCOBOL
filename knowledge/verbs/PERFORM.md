---
name: PERFORM
supported_clauses:
  - VARYING
  - UNTIL
  - TIMES
unsupported_clauses: []
since_version: "1.0.0"
---
# PERFORM Statement

The `PERFORM` statement is used to execute one or more procedures and then return control to the next executable statement.

## Syntax
`PERFORM procedure-name-1 [{THROUGH | THRU} procedure-name-2]`
