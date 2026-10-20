<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Golden vectors of the group <-> JSON mapping

`group-json.v1.json` is the contract between the two implementations of WASM-005 R5/R6: the Rust one in `crates/cobolt-wire` (used by the interpreter, the backend and the compiler) and the one the WebAssembly generator emits into the program module. Each implementation runs **the same file**; a difference is a failing test, not a field in production. The rules each case pins are in `../mapping-rules.md` (rule ids in each case's `rules`).

The file is JSON, written by hand from the rules, never generated from an implementation.

## A case

```json
{
  "id": "alnum-basic",
  "rules": ["G1", "S1"],
  "title": "…",
  "layout": "       01 REC.\n           05 NAME PIC X(8).\n",
  "initial": [["REC.NAME", "ANNA"]],
  "blank": [["REC.ITEM(4)", "40"]],
  "json": "{\"NAME\":\"ANNA\"}",
  "encode_error": "odo-out-of-range", "encode_error_path": "REC.ITEM",
  "layout_error": "share a name",
  "reads": [
    {"json": "{\"NAME\":\"Bo\"}", "changes": [["REC.NAME", "Bo"]]},
    {"json": "{\"NAME\":12}", "error": "wrong-type", "path": "REC.NAME"}
  ]
}
```

- **`layout`** — a DATA DIVISION fragment in **fixed format** (the column rule of a COBOL-85 source: `01` from column 8, subordinate items from column 12), the first item of which is the argument. Run it through the project's own lexer and parser (both implementations share the front end) inside a program with a `WORKING-STORAGE SECTION`; the argument is the first `01`/`77` item.
- **`layout_error`** — the layout must be refused, and the message must contain this text (an implementation is free to word the rest). No other field is present.
- **`initial`** — values set on the freshly initialised group (VALUE clauses, else zeros and spaces) before anything else, as `[path, text]`. A `path` is a **dumped path**: `REC.NAME`, `REC.LINES(2).QTY` (1-based subscripts), `REC.FILLER#3` for an unnamed item (its 1-based position among its group's children). `text` of a number must be **canonical** (`12.50`); text is set as given and padded (or right-aligned for `JUSTIFIED RIGHT`).
- **`json`** — what encoding `initial`'s state must produce, byte for byte. When present the case also pins the **round trip**: decoding this text into the `blank` state (default: the initial state; `blank` overrides leaves, used where the wire does not carry a table's storage past its count) and dumping it must equal the dump of `initial`.
- **`encode_error`** / **`encode_error_path`** — encoding `initial` must fail with this error code at this path.
- **`reads`** — each entry is decoded **independently, starting from `initial`'s state**.
  - `changes` — the read is accepted; the resulting state equals the initial state with these leaves replaced, **every other leaf unchanged**. `[path, text]` as above.
  - `error` + `path` — the read is refused with this error code and this path (codes are the closed list at the end of `mapping-rules.md`), and the state is untouched.
  - `json` may be any text, including invalid JSON; the empty string is a read too.

## State comparison

A state is the list of every elementary leaf in declaration order (`REDEFINES` overlays left out), each as `(path, text)`. **Compare with trailing spaces trimmed**; leading spaces are significant.

## Running them

- Reference: `cargo test -p cobolt-wire --test vectors`.
- The interpreter's adapter (the same vectors through the environment): `cargo test -p cobolt-runtime --test test_group_json_vectors`.
- The generator (`cobolt-wasm`): its own test loads this file with `include_str!` the same way and drives the emitted module's encode/decode entry points; it must print the same tally as the reference (cases, encodings, round trips, reads accepted, reads refused, layouts refused).

Adding a case: write it by hand from the rules, run all three, and if one disagrees decide **in `../mapping-rules.md`** which is right. Never change a vector to match an implementation without changing the rule.
