<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

---
name: rustcobol-types
description: >-
  The RustCOBOL type system — level numbers, the PICTURE clause (every symbol,
  digit counting, V/S/P rules, editing), USAGE (DISPLAY/COMP/COMP-3/COMP-5/…),
  numeric scale and ranges, VALUE rules, when a PIC is required vs forbidden, the
  types of control `::` properties and event LINKAGE, and EVERY deviation from the
  COBOL-85 standard. Load and obey this whenever you declare a data item, choose a
  PICTURE/USAGE, define an indexed-file record, or read/write a control property.
  Never emit a data item whose type is not valid under these rules.
---

# RustCOBOL type system (agent skill) — authoritative

You generate/edit COBOL for **RustCOBOL** = COBOL-85 **plus** a small set of
extensions. This skill is the **type contract**. It is CRITICAL: if you declare an
item with an invalid PICTURE, an impossible USAGE, an out-of-range level, or a
missing-but-required PICTURE, the code is rejected by the validator and the change
fails. Treat every rule here as a hard constraint, not a suggestion. When in
doubt, choose the **most standard COBOL-85** form.

The single most important habit: **before emitting any data item, mentally verify
`level` + `name` + `PICTURE?` + `USAGE?` against §1–§4 and the checklist in §11.**

---

## 1. Level numbers — exactly which are legal

| Level | Meaning | PICTURE? |
|------|---------|----------|
| `01` | Record / top group or a standalone elementary item | group: no · elementary: **required** |
| `02`–`49` | Subordinate group or elementary item | group: no · elementary: **required** |
| `66` | `RENAMES` regrouping (`66 NEW RENAMES A THRU B`) | **never** (not elementary data) |
| `77` | Standalone elementary item (no subordinates) | **required** |
| `88` | Condition-name (`88 IS-OK VALUE "Y".`) attached to the item above | **never** (not data) |

- Write levels **zero-padded to two digits** (`01`, `05`, `77`), Area-A only for
  `01`/`77` (column 8); subordinates indent under their parent.
- **`78` (constant) is NOT COBOL-85.** It is a COBOL-2002/vendor extension. Do NOT
  use `78` unless the developer explicitly asks; **never** use it in an indexed
  file record (the record validator rejects it — see §9). Prefer a normal item
  with a `VALUE` clause instead of an `78` constant.
- Group vs elementary is decided **structurally**: an item is a *group* iff a
  higher-numbered item follows it at a deeper level; otherwise it is *elementary*
  and MUST carry a PICTURE (or a no-PIC USAGE from §3).

---

## 2. The PICTURE clause — every symbol and how width is counted

`PIC` (or `PICTURE`) defines category + size. Categories (as classified by the
parser's `analyze_pic`):

| Category | Trigger | Example |
|---|---|---|
| **Alphabetic** | only `A` (no `9`) | `PIC A(10)` |
| **Numeric** | `9`/`S` and no `X`, no editing symbol | `PIC 9(5)`, `PIC S9(7)V99` |
| **Alphanumeric** | contains `X` (no editing symbol) | `PIC X(30)` |
| **Numeric-edited** | numeric + an editing symbol | `PIC ZZ,ZZ9.99`, `PIC -9(6)` |
| **Alphanumeric-edited** | `X` + `B`/`0`/`/` insertion | `PIC XXBXXBXXXX` |

### Data symbols (count toward width)
- `9` — one decimal digit position (`0`–`9`).
- `X` — one character position (any byte).
- `A` — one alphabetic position (letter or space).

### Non-counting / modifier symbols (numeric only)
- `S` — operational **sign**. Leading, at most one, **does not add a digit**.
  Makes the item signed (can hold negatives). No `S` ⇒ unsigned.
- `V` — **implied** decimal point. At most one, **occupies no character**. Splits
  integer digits (left) from fractional digits (right). `PIC 9(5)V99` = 5 integer
  + 2 fraction digits, scale = 2.
- `P` — decimal scaling position (assumed digit, holds no data). Rare; avoid
  unless the developer needs it.
- `(n)` — repetition of the preceding symbol: `9(5)` ≡ `99999`, `X(30)` ≡ thirty
  `X`. Always prefer the parenthesised form for width > 3.

### Editing symbols (make the item *edited* — output/display only)
`Z` (zero-suppress) · `*` (asterisk/check-protect) · `B` (blank insert) · `0`
(zero insert) · `/` (slash insert) · `,` (comma insert) · `.` (actual decimal
point) · `+` / `-` (sign, fixed or floating) · `$` (currency) · `CR` / `DB`
(credit/debit).
- **Edited items are for formatting a value into a display string. Never compute
  with them and never use them as a receiving field for arithmetic.** Use a plain
  `9`/`S9`/`V` numeric for arithmetic, then `MOVE` it into the edited item to show it.

### Hard PICTURE rules — reject anything that violates these
1. **Do not mix classes.** No `9` together with `X`, no `A` with `9` (except an
   `X`-based edited picture). A field is numeric **or** alphanumeric **or**
   alphabetic — pick one.
2. `S`, `V`, `P` are **numeric-only** — never in an `X`/`A` picture.
3. At most **one** `S` (leading) and at most **one** `V` per numeric picture.
4. **Integer + fractional digit count ≤ 18** for a portable COBOL-85 numeric.
   (RustCOBOL stores numerics as a 128-bit mantissa and will accept up to ~38, but
   that is a **deviation** — stay ≤ 18 unless the developer explicitly wants more.)
5. Alphanumeric/alphabetic width may be large (`PIC X(4096)`, up to `X(32767)`).
6. An elementary item's PICTURE is **mandatory** unless its USAGE is `INDEX`,
   `POINTER`, or `OBJECT REFERENCE` (§3). Group items and `66`/`88` take none.
7. The actual decimal point `.` in an **edited** picture is a display point; the
   arithmetic point is `V` in the underlying numeric.

---

## 3. USAGE — representation, sign, and PICTURE requirement

`USAGE` sets the machine representation. Supported values (from the `Usage` enum):

| USAGE (write as) | Representation | Needs PIC? | Notes / standard |
|---|---|---|---|
| `DISPLAY` (default) | one byte per character/digit | numeric/alnum: **yes** | COBOL-85. Omit `USAGE` to get it. |
| `BINARY` / `COMP` / `COMP-4` | native binary integer | **yes** (numeric) | COBOL-85. `COMP` = `BINARY` here. |
| `COMP-3` / `PACKED-DECIMAL` | packed BCD | **yes** (numeric) | COBOL-85. Two digits/byte + sign nibble. |
| `COMP-5` | native binary, no sign truncation to PIC | **yes** (numeric) | **Extension** (COBOL-2002/vendor). |
| `COMP-1` | 32-bit IEEE float | **no PIC** (it defines its own size) | **Extension** — floating point. |
| `COMP-2` | 64-bit IEEE float | **no PIC** | **Extension** — floating point. |
| `INDEX` | table index register | **never** a PIC | COBOL-85 (`USAGE INDEX`). |
| `POINTER` | memory address | **never** a PIC | Extension (COBOL-2002). |
| `OBJECT REFERENCE <class>` | object handle (Rust-FFI bridge) | **never** a PIC | **Extension** (COBOL-2002 / spec 005). |

Rules:
- A `COMP`/`COMP-3`/`COMP-4`/`COMP-5`/`BINARY` item **must** have a **numeric**
  PICTURE (`9`/`S9`/`V`). Giving it an `X`/`A`/edited picture is invalid.
- `COMP-1`/`COMP-2` (floats) take **no PICTURE** — writing one is invalid.
- `INDEX`/`POINTER`/`OBJECT REFERENCE` take **no PICTURE**.
- Prefer plain `DISPLAY` or `COMP-3` for money, `COMP`/`BINARY` for counters/loop
  indices, unless the developer specifies otherwise. Reach for `COMP-1`/`COMP-2`
  only when the developer explicitly needs floating point.

Typical `COMP`/`BINARY` storage widths (guidance): `9(1–4)` → 2 bytes, `9(5–9)` →
4 bytes, `9(10–18)` → 8 bytes.

---

## 4. Numeric semantics — scale, sign, ranges

- A numeric value = **integer mantissa × 10^(−decimals)**. `decimals` comes from
  the digits right of `V`. `PIC 9(5)`→scale 0; `PIC 9(5)V99`→scale 2;
  `PIC S9(4)V9`→scale 1, signed.
- **Unsigned** (no `S`) holds `0 … 10^digits − 1`. **Signed** (`S`) holds the
  negative range too.
- A receiving field silently **truncates** to its declared integer/fraction
  digits — size the field to the values it must hold. Choose `S` whenever a value
  can go negative; an unsigned field stores the absolute value.
- Do arithmetic on plain numerics; format for display via a numeric-**edited**
  picture (§2).

---

## 5. VALUE clause

- `VALUE` gives an initial value; its literal **must match the item's category**:
  a quoted string for `X`/`A` (`VALUE "N/A"`), a number for numeric
  (`VALUE 0`, `VALUE -1`, `VALUE 3.14`), a figurative constant where valid
  (`VALUE SPACES`, `VALUE ZEROS`).
- A numeric `VALUE` must fit the PICTURE's digits/scale. `VALUE "abc"` on a
  numeric item, or `VALUE 5` on an `X` item, is invalid.
- Under `SPECIAL-NAMES. DECIMAL-POINT IS COMMA` numeric literals use `,` as the
  decimal separator (`VALUE 1234,50`). Only use comma decimals if the form's
  SPECIAL-NAMES declares it.

---

## 6. Control property types (the `::` operator)

Reading/writing `Control::Property` (see the `rustcobol-extensions` skill for
syntax). Match the **value type** to the property:

- **Numeric** properties (`Value` on Slider/Spinner/ProgressBar/NumericUpDown,
  geometry like `Left`/`Top`/`Width`/`Height`, `FontSize`, and Neumorphic depth
  like `ShadowBlurStrength`) are algebraic — compare/assign with plain numeric
  literals or numeric items, **no intermediate PIC needed**
  (`IF Slider-1::Value > 50`, `MOVE 12 TO Label-1::FontSize`).
- **String** properties (`Caption`, `Text`) take alphanumeric values / quoted
  literals or `X`-picture items.
- **Colour** properties (`BackgroundColor`, `ForegroundColor`, …) are `#RRGGBB`
  **string** literals: `MOVE "#008000" TO Label-1::ForegroundColor`. Never assign
  a raw number to a colour.
- **Boolean-style** properties (`Visible`, `Enabled`, `Checked`) are set with the
  control's own values — assign `1`/`0` (or use the control's `Show`/`Hide`,
  `Check`/`Uncheck` methods) as documented for that control; don't invent
  `TRUE`/`FALSE` literals.
- Do not declare a `PIC` to hold a property; read it straight into a MOVE/IF/
  COMPUTE, or into an item whose category matches (numeric prop → numeric item,
  text prop → `X` item).

---

## 7. Event data / LINKAGE types

- Handlers receive their event's items in `LINKAGE SECTION`, bound by
  `PROCEDURE DIVISION USING …`. Use **only** the LINKAGE items the CONTEXT lists;
  do not invent them. Most events deliver none → empty LINKAGE, plain
  `PROCEDURE DIVISION.`
- Repeating-group (array) member handlers receive the 1-based firing index as
  **`CONTROL-ARRAY-INDEX PIC S9(4) COMP-5`** — declare/reference it exactly with
  that picture and usage.

---

## 8. Deviations from the COBOL-85 standard — be explicit and aware

Always know whether a construct is **standard COBOL-85**, a **supported
extension**, or **not supported**. Emit standard forms by default; use an
extension only when the request needs it, and never emit an unsupported one.

**Supported extensions (beyond COBOL-85 — fine to use when needed):**
- The **`::` control property/method operator** for GUI controls (RustCOBOL GUI).
  The low-level GUI runtime `CALL`s exist but are not yours to write.
- Handler/procedure bodies as **nested programs without `IDENTIFICATION`/
  `PROGRAM-ID`** (the IDE supplies them — see `rustcobol-extensions`).
- **Floating point** `COMP-1`/`COMP-2`, and **`COMP-5`** (COBOL-2002/vendor).
- **`USAGE POINTER`** and **`OBJECT REFERENCE`** (COBOL-2002 / Rust-FFI bridge).
- **`78` constants** — parser-level only; avoid unless asked, and never in indexed
  records.
- **`DECIMAL-POINT IS COMMA`** via SPECIAL-NAMES.
- 128-bit numeric mantissa (numerics may exceed 18 digits) — a capacity deviation;
  keep to ≤ 18 digits for standard conformance unless told otherwise.

**Stay standard / avoid unless explicitly requested:** vendor-specific pictures,
non-English identifiers/comments/literals (never), and any USAGE/PICTURE
combination not listed in §2–§3.

If you are unsure whether something is supported, **do not emit it** — choose the
plain COBOL-85 equivalent and, if truly necessary, leave a `*>` comment noting the
assumption for the developer.

---

## 9. Indexed-file record fields — STRICTER rules

The indexed-file record editor validates with **no tolerance** (see the record
validator). When defining or editing an indexed record layout:
- **Every elementary field MUST carry a `PIC`/`PICTURE`.** A missing PICTURE is a
  hard error (the one exception, `USAGE INDEX`/`POINTER`, does not belong in a
  stored record).
- **Levels allowed: `01`–`49`, `66`, `77`, `88` only.** `78` is **rejected** here.
- Only conformant **COBOL-85** syntax is accepted — no vendor picture symbols, no
  malformed clauses. `USAGE` may be `DISPLAY`, `COMP`, `COMP-3`, `COMP-4`,
  `BINARY`, `PACKED-DECIMAL`, `INDEX`, or `POINTER`.
- Keep field names valid COBOL data-names (letters/digits/hyphens, not starting or
  ending with a hyphen, English).

---

## 10. Worked examples (all valid)

```cobol
       01  WS-CUSTOMER.
           05  WS-CUST-ID      PIC 9(6).                *> unsigned 6-digit
           05  WS-NAME         PIC X(30).               *> 30 chars
           05  WS-BALANCE      PIC S9(7)V99 COMP-3.     *> signed money, packed
           05  WS-RATE         PIC S9(1)V9(4).          *> signed, 4 decimals
           05  WS-COUNT        PIC 9(4) COMP.           *> binary counter
           05  WS-FLAG         PIC X.                   *> single char
               88  FLAG-ON      VALUE "Y".              *> condition-name, no PIC
       77  WS-BAL-EDITED       PIC $,$$$,$$9.99.         *> edited: display only
       77  WS-PI               USAGE COMP-2.             *> 64-bit float, NO pic
```

Invalid — never emit:
```cobol
       05  BAD-1  PIC 9X(3).            *> mixes numeric and alphanumeric
       05  BAD-2  PIC X(5)V99.          *> V/S/P are numeric-only
       05  BAD-3  PIC 9(20).            *> > 18 digits (non-standard)
       05  BAD-4  USAGE COMP-1 PIC 9(5).*> COMP-1/2 take no PICTURE
       05  BAD-5.                       *> elementary item with no PICTURE
       78  MAX-ROWS VALUE 100.          *> 78 not COBOL-85 (and never in a record)
```

---

## 11. Pre-emit checklist (run for EVERY data item)

1. Is the **level** one of `01`–`49`, `66`, `77`, `88`? (Records: same, no `78`.)
2. Is it a **group** (a deeper item follows) → **no** PICTURE, no USAGE-of-value?
3. Is it **elementary** → does it have a **PICTURE** (or a no-PIC USAGE: INDEX/
   POINTER/OBJECT REFERENCE/COMP-1/COMP-2)?
4. Does the **PICTURE mix classes** or put `S`/`V`/`P` in a non-numeric? → fix.
5. Numeric **digits ≤ 18**? Signed (`S`) if it can be negative?
6. Is the **USAGE ↔ PICTURE** pair legal (numeric PIC for COMP/COMP-3/…; no PIC for
   COMP-1/COMP-2/INDEX/POINTER)?
7. Does any **VALUE** match the item's category and fit its size?
8. Is every identifier **English** and a valid COBOL data-name?
9. Are you using an **extension** (§8)? Only if requested; otherwise use the
   plain COBOL-85 form.

If any answer is wrong, correct the declaration before emitting it. Do not produce
an item you cannot verify against this list.
