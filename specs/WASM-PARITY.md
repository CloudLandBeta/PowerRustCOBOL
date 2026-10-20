# WASM parity ledger — where a generated module and the interpreter differ

Every difference between the WebAssembly line (2.x.x) and the interpreter (1.x.x), with who is right. A difference that is the **interpreter's** defect is a fix for `main` (GOLDEN RULE #5) and is forward-ported; it is listed here until the operator decides. A difference that is deliberate stays here as the record of why.

| # | Construct | Module | Interpreter | Who is right | Status |
|---|---|---|---|---|---|
| 1 | `IF N = ZERO` where `N` is a numeric item with `BLANK WHEN ZERO` holding zero | true (the item is the number zero; the clause changes how it is shown) | false (the item is compared as its blank display text) | the module (COBOL-85 5.4) | **open — report to the operator**: a 1.x.x fix in `Interpreter::as_comparand` (`interpreter.rs`), which should substitute the blank text only against a character operand, as the module does. No NIST member exercises it. |
| 2 | `BLANK WHEN ZERO` numeric-edited item receiving a value that is zero only after truncation to the picture's decimals (`0.001` into `Z9.99`) | blanks | prints `0.00` (tests the value before truncation) | the module (the item's value is zero) | open, same fix as #1's family; no NIST member exercises it |
| 3 | Conformance builds: a record-sequential file | one record per line | fixed-length records, no separator | neither — a test-host choice; the module has no files in a browser | by design; it is why the harness tallies 4 612 `PASS` markers against the interpreter's 4 614 while every program's own summary line is identical |
| 4 | `FUNCTION` calls, nested programs, `CALL … LINKAGE`, `EXEC`, control members | refused at build time with the construct named | run | — | open: IF and IC modules (GOLDEN RULE #9) |
