<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Bug Tracker

> **How it works**
> - `tools/check_bugs.sh` (or the daily scheduled scan) runs `cargo check --workspace` and
>   parses every `error[…]` line into this file.
> - A new bug gets a unique ID (`BUG-NNN`), the detection date, the affected crate, the Rust
>   error code, and a one-line summary.
> - When a bug is fixed, it is moved to the **Resolved** table and the fix is summarised in
>   `CHANGELOG.md` under the next version entry.
> - Run `./tools/check_bugs.sh` manually or from a scheduled job; it exits `1` when
>   open bugs remain (suitable for CI or an external notifier you wire up).

---

## Open Bugs

| ID | Detected | Crate | Error | Summary |
|----|----------|-------|-------|---------|
| BUG-001 | 2026-09-14 | cobolt-runtime | runtime | `USAGE` is parsed and then ignored: `COMP-3`, `PACKED-DECIMAL`, `BINARY`, `COMP` and `COMP-5` items get DISPLAY storage, so every record holding one is byte-incompatible with other COBOL implementations. |
| BUG-002 | 2026-09-14 | cobolt-runtime | runtime | `FUNCTION LENGTH` is value-derived, not declaration-derived, on numeric items: `PIC 9(7)V99` returns 10 with a `VALUE` and 4 without, where the standard requires 9 in both cases. |

### BUG-001 — `USAGE` is accepted and discarded

**Not a compiler error**, so `check_bugs.sh` did not find it and never will — it
was found by `/doc-audit` on `docs/cobol85-supported-syntax-en.md`, whose
line 980 claims ✅ for the whole `USAGE` list.

**Measured, differentially, at 1.70.12.** One record, two compilers:

```cobol
01  OUT-REC.
    05 R-COMP3  PIC 9(7)V99 COMP-3.
    05 R-BIN    PIC S9(4)   COMP.
```

| | Bytes written |
|---|---|
| GnuCOBOL 3.2 | **7** — 5 packed + 2 binary (correct) |
| rcrun | **13** — 9 + 4 DISPLAY characters |

**Root cause.** `Usage` has 11 variants; the runtime reads the field in five
places, covering only `Display`, `Comp1`/`Comp2` and `ObjectReference`.
`Comp3`, `PackedDecimal`, `Binary`, `Comp` and `Comp5` are lexed, parsed into the
AST (`cobolt-parser/src/data.rs:1104`) and acknowledged by the semantic analyser
(`cobolt-semantic/src/type_checker.rs:106`) — then never consulted.
`compute_layout`'s `walk()` (`cobolt-runtime/src/files.rs:227`) sizes every field
as `pic.digits + pic.decimals + sep` and never reads `usage`.

**Why nothing caught it.** COBOL-85 leaves `PACKED-DECIMAL`'s representation
implementor-defined, so CCVS85 asserts arithmetic behaviour rather than bytes —
and treating COMP-3 as DISPLAY gives correct arithmetic. NC therefore stands at
95/95 legitimately. This is an **interop** defect, not a conformance one: a data
file from another COBOL shop is unreadable, and one written here is unreadable by
them. No runtime test covers it.

**Related — BUG-002**, from the same audit, and they share a fix surface: once
`FUNCTION LENGTH` consults the declaration instead of the value, it must also
honour `USAGE` to return 5 for a `COMP-3` item. Fixing either alone leaves the
other half wrong.

**Doc consequence, parked.** `docs/cobol85-supported-syntax-en.md:980` is wrong
and nothing in that document warns the reader. Parked in `NIST/progress.json`
rather than fixed on a `z` bump, because GOLDEN RULE #8 charges five translations
per edit to that canonical.

### BUG-002 — `FUNCTION LENGTH` measures the value, not the declaration

**Not a compiler error either**, and found the same way — by `/doc-audit` on
`docs/cobol85-supported-syntax-en.md`, whose line 842 claims "The **complete
COBOL-85 standard intrinsic set** is implemented".

**Measured, differentially, at 1.70.17:**

| Declaration | rcrun | GnuCOBOL 3.2 | Standard |
|---|---|---|---|
| `PIC X(10)` | 10 | 10 | 10 ✅ |
| `PIC X(10) VALUE "AB"` | 10 | 10 | 10 ✅ |
| `PIC 9(7)V99 VALUE 1234567.89` | **10** | 9 | 9 ❌ |
| `PIC 9(7)V99` (no `VALUE`) | **4** | 9 | 9 ❌ |

**Root cause** — `cobolt-runtime/src/interpreter.rs:13642`:

```rust
"LENGTH" => {
    let v = self.eval_expr(&args[0], span)?;          // collapses the item to a VALUE
    let len = match &v {
        CobolValue::String { bytes, .. } => bytes.len(),   // alphanumeric: correct by luck
        _ => v.as_display_string().len(),                  // numeric: length of the RENDERED value
    };
```

`eval_expr` reduces the identifier to a value before anything can look at its
PICTURE. For a `CobolValue::String` the stored byte count happens to equal the
declared width, which is why `PIC X(10)` is right — it is correct by coincidence,
not by construction. A numeric falls to `as_display_string().len()`, so it
measures whatever the item currently holds: `"1234567.89"` is 10 characters, and
an uninitialised item renders 4.

COBOL-85 defines `FUNCTION LENGTH` as the number of character positions **in the
argument** — a property of the declaration, constant at run time for a
fixed-size item. It must be read from the PICTURE (and, per BUG-001, the
`USAGE`), never from the current value.

⚠️ **Do not "fix" the other two `LENGTH` sites.** `interpreter.rs:11969` and
`:12179` implement the member-call extension `x::Length()` / `x::Len()`, which
operates on a **value** and returns its character count. That is its intended
contract and it is correct. Only the `FUNCTION LENGTH` arm at `:13642` is wrong.

**Why nothing caught it.** The NIST **IF (Intrinsic functions)** module stands at
**45/45 on both axes** — the suite does not exercise `LENGTH` against a numeric
item. That is a statement about coverage, not a defect in the suite.

**Doc consequence, parked** alongside BUG-001's, in
`NIST/progress.json` → `syntax-doc-audit-findings-1.70.17`.

---

## Resolved Bugs

| ID | Detected | Fixed | Crate | Error | Summary | Fix |
|----|----------|-------|-------|-------|---------|-----|

_None yet._
