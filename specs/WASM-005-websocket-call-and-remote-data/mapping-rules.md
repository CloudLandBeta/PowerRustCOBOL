<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# The COBOL group <-> JSON mapping — the normative rules

- **Status:** normative for WASM-005 R5, R5b and R6. Written 2026-10-20 by the implementation of M2; the reference implementation is `crates/cobolt-wire` (`layout`, `node`, `mapping`, `json`, `number`), the golden vectors are `vectors/group-json.v1.json`.
- **Who implements it:** the interpreter and the backend (through `cobolt-wire`) and the WebAssembly generator (`cobolt-wasm`, its own implementation inside the emitted module). **Both must pass the same vectors** (`vectors/README.md`); where this table and a vector disagree, that is a defect in one of them and is settled here, in writing, before either implementation changes.
- **Decisions this file takes** (the operator was away; each is recorded in `plan.md` with its alternative): the unit of a `PIC X` length is the **byte** (Q4), a key is the data-name **as declared** (Q3), an elementary argument is its **bare scalar** (Q1), `REDEFINES` writes the redefined item only (Q5).

A **layout** is the declaration of an argument (a `01`/`77` item, or any group below it). A **state** is the value of that group. *Encoding* turns a state into JSON text; *decoding* turns JSON text into a new state built from the current one. Neither ever changes the state it is given.

## G — groups and names

| # | Rule |
|---|---|
| **G1** | A group is a JSON **object**. Its members are written in **declaration order**. A member's key is its data-name **as declared** — spelling preserved, case included (`Cust-Name` is `"Cust-Name"`). Keys are compared exactly (case-sensitively) when reading. |
| **G2** | Not members, never written, and a key naming one is an **unknown key** when reading: an unnamed item and any item named `FILLER`; every **88-level** condition-name; a level-**66** `RENAMES`; an item declared with **`REDEFINES`**, with everything below it. Only the redefined (first) item travels. |
| **G3** | Two members of one group whose names are equal ignoring case cannot be told apart in an object: the layout is **refused** (a Check-time error naming both). |
| **G4** | The text written is compact: no whitespace outside strings; `,` and `:` unspaced. The text read is any RFC 8259 JSON (J1). |
| **E1** | An **elementary** argument (a `77`, or an elementary `01`) is its **bare scalar** — `12.50`, `"hello"` — not an object. A group is an object. The top-level name of the argument is never on the wire (the position in the call identifies the parameter); only the names of members are. |

## T — tables

| # | Rule |
|---|---|
| **T1** | `OCCURS n` is a JSON **array** of n elements, each written as the item would be without the `OCCURS` (a scalar, or an object for a group item). |
| **T2** | `OCCURS min TO max DEPENDING ON obj` is an array of the **current count**: the value of `obj`. The count must lie in `min..max`; otherwise encoding fails (`odo-out-of-range`). `obj` must be a non-table **integer** (scale 0) item declared **earlier, inside the argument group**; it is found by name, in the group holding the table and then in each enclosing group, going down through groups but never into a table; a layout where it is not found is refused (Check time). Storage past the count is not on the wire. |
| **T3** | A table inside a table element is a nested array; a table below a table of groups is an array inside each element's object. A count item found inside the same table element is that element's own. |
| **T4** | A table of groups is an array of objects. |

## N — numbers

A numeric item travels as its **decimal text** with its PICTURE scale. No binary floating-point value is ever built.

| # | Rule |
|---|---|
| **N1** | **Canonical text** (what is written and what the vectors compare): `-?D+(.D+)?` — no `+`, no exponent, no leading zeros except the single `0` before the point, **exactly `scale` fraction digits** (and no point when the scale is 0), and **no sign on zero** (`0.00`, never `-0.00`). `PIC S9(5)V99` holding 12.5 is `12.50`; `PIC SV999` holding 0.05 is `0.050`; `PIC 9(3)` holding 7 is `7`. |
| **N2** | **Reading.** The JSON number token is taken exactly as written. It is accepted iff its value is exactly representable in the item: after dropping fraction zeros it has at most `scale` fraction digits (`1.500` and `1e2` are fine for scales 2 and 0, `1.505` is not), at most `digits` integer digits, and a minus sign only if the item is signed (`-0` is zero and is accepted by any item). Anything else is `overflow`. A string, boolean, `null`, array or object is `wrong-type`. An exponent beyond ±1000 is `overflow` without being expanded. |
| **N3** | **`USAGE` is not on the wire.** `DISPLAY`, `COMP`, `COMP-4`, `COMP-5`, `COMP-3` and `PACKED-DECIMAL` items are all their value, bounded by the **PICTURE** (`PIC S9(4) COMP` holds ±9999 on the wire, whatever its binary capacity). `COMP-1`/`COMP-2` (binary floating point), `INDEX`, `POINTER`, `OBJECT REFERENCE` and a `P` scaling position have no wire form: the layout is refused. `SIGN` clauses change storage only. |
| **N4** | Numbers of up to **38 digits** (the width of the interpreter's `i128` mantissa) are written and read exactly; a number a JavaScript `Number` cannot hold (beyond 2^53) is still written in full, and the module reads it as a decimal. A picture of more than 38 digits is refused. |

## S — text

| # | Rule |
|---|---|
| **S1** | **Writing.** `PIC X`/`PIC A` is a JSON string. The item's bytes, with **trailing spaces (0x20) trimmed**, must be valid UTF-8 (`not-utf8` otherwise: a `HIGH-VALUE` byte cannot be sent). All spaces is `""`. Leading spaces are kept. |
| **S2** | **Reading.** The string's UTF-8 encoding must be at most **n bytes** for `PIC X(n)` (`too-long` otherwise — never truncated); it is stored left-aligned and padded with spaces, or right-aligned for `JUSTIFIED RIGHT`. `""` stores all spaces. A multi-byte character costs its bytes: `PIC X(6)` holds `"çãç"` (6 bytes) but not `"ççça"` (7). |
| **S3** | **Escapes written:** `"` and `\` are escaped; U+0000–U+001F are written `\b \t \n \f \r` or `\u00xx` (lower-case hex); everything else — `/` and all non-ASCII — is written as is. **Read:** every RFC 8259 escape, surrogate pairs combined, a lone surrogate is a syntax error. |
| **S5** | `PIC N(n)` (national): a string of at most n **UTF-16 code units**. `PIC U(n)` (UTF-8): at most n **characters**, and at most the item's bytes when it was declared `BYTE-LENGTH`. Both are stored padded with spaces. |
| **S6** | A numeric-edited or alphanumeric-edited picture travels as its **edited image**, a JSON string of at most the picture's width in bytes, trailing spaces trimmed. Reading stores the characters as given; it does **not** re-edit them. |

## R — reading

| # | Rule |
|---|---|
| **R1** | **By name, not by position.** Object members may come in any order. A key absent from the text leaves its item **unchanged**. An unknown key is `unknown-key`; a repeated key is `duplicate-key`; a value of the wrong JSON type is `wrong-type` (`null` is never a value). |
| **R2** | **Arrays.** More elements than `OCCURS` (or `max`) is `too-many-elements`. A **fixed** table given fewer leaves the elements not given unchanged. A **variable** table given fewer than `min` is `too-few-elements`. Reading an array into a variable table **sets the count item** to the array's length; if the text also gives the count item it must equal the length (`odo-mismatch`). A count the text sets alone must still lie in `min..max` (`odo-out-of-range`), and must fit its own PICTURE. |
| **R3** | **Atomic.** An error names the item (`ORDER.LINES(3).QTY`, 1-based subscripts) and **nothing is written**: decoding builds a new state and the caller replaces its group only when the whole text was accepted. A call with several groups writes all of them or none. |
| **R4** | A group written and read back is the same group (the vectors check this for every encoding); the elements of a variable table past its count are not on the wire and are therefore not carried. |

## J — the text

| # | Rule |
|---|---|
| **J1** | RFC 8259, nothing guessed: no comments, no trailing commas, no single quotes, no `NaN`/`Infinity`, no leading zeros, no `+1`, no `.5`, no raw control characters in strings, no text after the value, no repeated key. Nesting deeper than **64** and a text longer than the configured limit (default 16 MiB; the backend's frame limit is smaller) are `syntax`. Leading and trailing whitespace is allowed. |

## The error list (closed)

`syntax` · `duplicate-key` · `unknown-key` · `wrong-type` · `overflow` · `too-long` · `too-many-elements` · `too-few-elements` · `not-utf8` · `odo-mismatch` · `odo-out-of-range` · `internal` (the state does not match its layout — a defect of the caller, never produced by a conforming implementation on its own output).

Every error carries the **path** of the item (the root name for `syntax`/`duplicate-key`/a wrong top-level type) and an English message for logs.

## What the vectors compare

A vector's *state* is the list of `(path, text)` of every elementary item in declaration order, `REDEFINES` overlays left out, an unnamed item written `FILLER#k` (its 1-based position in its group). The text is the stored text — canonical decimal for a number, the characters for text — **compared with trailing spaces trimmed**, which keeps padding an implementation detail while leaving leading spaces (JUSTIFIED RIGHT) significant.

## Open points this file does not settle

- The width of a `PIC X` item in **characters** for non-ASCII text (WASM-020, accessibility and internationalisation) — the byte rule above is what the interpreter's storage does today and is conservative for a browser module.
- Binary payloads and large tables (WASM-005 Q9: one JSON text frame per call; chunking and file transfer are WASM-014).
