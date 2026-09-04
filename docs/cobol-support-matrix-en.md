<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL Support Matrix

**What this document is for:** one scannable place that answers *"does
PowerRustCOBOL do X, and is X standard COBOL or something this platform adds?"*
Every capability is a row. No prose lists — if a thing is supported, it has a
line you can point at.

This is the **overview**. Two companions carry the detail:

| Document | What it answers |
|---|---|
| [`cobol85-supported-syntax-en.md`](cobol85-supported-syntax-en.md) | **Which spelling** of each statement the lexer/parser/runtime actually accept, and the NIST CCVS85 conformance scoreboard |
| [`cobol85-verb-test-matrix-en.md`](cobol85-verb-test-matrix-en.md) | **What to test** for each verb |
| [`developers-guide-en.md`](developers-guide-en.md) | How to build applications with all of it |

---

## How to read the tables

Each capability row is marked against three origins, then given a status.

| Column | Meaning |
|---|---|
| **85** | Defined by **COBOL-85** (ANSI X3.23-1985, including the 1989 intrinsic-function amendment where noted) |
| **20xx** | Defined by a **later ISO standard** — COBOL 2002 / 2014 / 2023, and what is currently drafted toward 2026 |
| **PRC** | A **PowerRustCOBOL extension** — not in any COBOL standard |
| **Status** | What this implementation does with it |

A capability can be marked in more than one origin column: a COBOL-85 feature
that a later standard extended is `●` in both, and the **Notes** column says
what the later standard added.

**Origin marks:** `●` defined here · `○` extended/clarified here · `—` not in
this standard.

**Status marks:** `✅` supported · `🚧` partial or simplified · `⛔` planned,
not yet implemented · `🚫` out of scope by design, will never be implemented.

> **Honesty note.** PowerRustCOBOL targets a practical, application-oriented
> subset plus visual RAD extensions. It is **not** a certified COBOL-85
> implementation. Conformance is *measured* against the official NIST CCVS85
> suite rather than asserted — see the
> [scoreboard](cobol85-supported-syntax-en.md).

---

## 1. Source format and program structure

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Fixed-form source, **relaxed** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **The default.** Sequence area and indicator column are honoured, but the line runs as far as the developer typed — no column-72 cut. Generated form `.cbl` and `EXEC RUST` blocks need this |
| Fixed-form source, **classic COBOL-85 reference format** (`--source-format=fixed`) | ● | ○ | — | ✅ | Every column rule applied: 1–6 sequence, 7 indicator (`*` `/` comment, `-` continuation, `D` debugging line), 8–72 source, **73–80 discarded**, standard continuation joining including a continued alphanumeric literal. What the NIST CCVS85 card-image suite is written in. **Chosen explicitly, never by detection** — applying these rules to source not written for them silently deletes code |
| Free-form source | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Source-format switch — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | Also `COBOLT_SOURCE_FORMAT`; `auto` inspects the first lines and never selects the strict format |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ | |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ | |
| DATA DIVISION | ● | ○ | — | ✅ | |
| PROCEDURE DIVISION | ● | ○ | — | ✅ | |
| Nested programs | ● | ○ | — | ✅ | |
| Multiple sequential program units in one file | ● | ○ | — | ✅ | |
| `COPY` / `REPLACE` copybooks | ● | ○ | — | ✅ | Pseudo-text and word replacement, nested `COPY`, `REPLACE OFF`; resolves `.cpy`/`.cbl`/`.cob` beside the source, case-insensitively |
| `REPOSITORY` paragraph | — | ● | ○ | ✅ | COBOL 2002 for classes; PowerRustCOBOL also binds **Rust FFI** types here |
| `EXEC RUST … END-EXEC` inline Rust | — | — | ● | ✅ | Compiled into the binary; errors are reported at the developer's own COBOL line and column |

## 2. Data division and data description

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ | |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ | |
| FILE SECTION | ● | ○ | — | ✅ | |
| SCREEN SECTION | ● | ○ | — | 🚧 | Extended `ACCEPT`/`DISPLAY` `AT`/`WITH` execute via ANSI in CLI mode; field-level screen editing is superseded by the visual form designer in GUI mode |
| COMMUNICATION SECTION (`CD`, message control) | ● | — | — | 🚫 | Teleprocessing; obsolete in later standards |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Out of scope by design |
| `PICTURE` X / A / 9 / S / V with `(n)` repetition | ● | ○ | — | ✅ | |
| Numeric-edited PICTURE (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Zero-suppression, check-protection, fixed and floating `$` and signs |
| `USAGE DISPLAY` | ● | ○ | — | ✅ | |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ | |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Floating point; a vendor extension standardised later as `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ | |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Native binary; vendor extension |
| `USAGE INDEX` | ● | ○ | — | ✅ | |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002; alias read **and** write |
| `OCCURS` fixed | ● | ○ | — | ✅ | |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ | |
| `INDEXED BY` | ● | ○ | — | ✅ | |
| Level numbers 01–49, 77 | ● | ○ | — | ✅ | |
| Level 66 `RENAMES` | ● | ○ | — | ✅ | |
| Level 88 condition-names | ● | ○ | — | ✅ | Including `SET … TO TRUE` |
| `VALUE` clause | ● | ○ | — | ✅ | |
| Group items, `FILLER` | ● | ○ | — | ✅ | |
| `REDEFINES` | ● | ○ | — | ✅ | |
| Figurative constants (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ | |

## 3. Procedure division — verbs

| Verb | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Group-subfield matching |
| `DISPLAY` | ● | ○ | — | ✅ | Numeric rendered at full PIC width |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ | |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | Multiple receivers, per-receiver `ROUNDED` |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Multiple receivers, per-receiver `ROUNDED` |
| `COMPUTE` | ● | ○ | — | ✅ | Multiple receivers, per-receiver `ROUNDED` |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ | |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ | |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ | |
| `PERFORM` inline, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ | |
| `PERFORM para VARYING` (out-of-line) | ● | ○ | — | ✅ | |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ | |
| `ALTER` | ● | ○ | — | ✅ | Obsolete element in COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Faithful semantics; obsolete in COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ | |
| `EXIT` | ● | ○ | — | ✅ | |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ | |
| `GOBACK` | — | ● | — | ✅ | Vendor extension standardised in COBOL 2002 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ | |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | COBOL 2002 pointers |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Category-aware, recurses groups |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ | |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | Combined `TALLYING REPLACING` |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Drives the table index, runs the first matching `WHEN`, else `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` and `RETURNING` are COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ | |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ | |
| `INVOKE` | — | ● | ○ | 🚧 | COBOL 2002 OO. Supported for **GUI and runtime objects and Rust FFI plugins**; user-defined class/method definitions are not implemented |
| `UNLOCK` | — | ● | — | 🚧 | Drives per-run record locks; not enforced across OS processes |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Program-controlled transactions on INDEXED files, with a real undo log |
| OO `CLASS-ID` / `METHOD-ID` definitions | — | ● | — | ⛔ | Planned |

## 4. Conditions and expressions

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Relation, class, sign and condition-name conditions | ● | ○ | — | ✅ | |
| Abbreviated combined relations, operator-prefixed (`a > 1 AND < 9`) | ● | ○ | — | ✅ | |
| Abbreviated combined relations, literal object (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ | |
| Abbreviated combined relations, identifier object (`a = b OR c`) | ● | ○ | — | ✅ | |
| Reference modification `item(start:length)` | ● | ○ | — | ✅ | Read **and** spliced write, on any operand |
| Runtime table subscripting `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Per-occurrence storage, variable subscripts |
| Qualified names `id OF/IN group` | ● | ○ | — | ✅ | A leaf declared under more than one group resolves to independent storage |
| COBOL-correct alphanumeric comparison (space-padded) | ● | ○ | — | ✅ | |
| **Exact fixed-point arithmetic** | ● | ○ | ○ | ✅ | `i128` integer mantissa, no `f64` round-trips: 18-digit standard and **31-digit extended** precision stay exact |
| Concise property expressions (`Output::Value`) | — | — | ● | ✅ | Get/set a control property inside a formula, with no temporary working-storage item |

### 4.1 Value methods on a data item

`item::Method(args)` calls a method on the **value of an ordinary data item** —
a `PIC X` field, a group, a table occurrence, a reference-modified slice or an
arithmetic expression — not just on a control. None of this is standard COBOL.

Usable anywhere an expression is: as a `MOVE` source, in a `COMPUTE`, inside a
condition, and inline in a `DISPLAY`. Methods **chain**:
`WS-TEXT::Trim()::Len()`.

| Method | Returns | Status | Notes |
|---|---|:--:|---|
| `Trim()` | text | ✅ | Leading and trailing spaces removed |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | text | ✅ | Three accepted spellings of one method |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | text | ✅ | |
| `Replace(from, to)` | text | ✅ | Every occurrence |
| `Len()` · `Length()` | numeric | ✅ | The **field's** length, so a `PIC X(20)` holding `hello` answers `20`. Chain `::Trim()::Len()` for the length of the content |
| `Split(sep)` | text | ✅ | The **first** field |
| `Split(sep)(n)` | text | ✅ | The *n*-th field, 1-based. The subscript is only accepted on a data-item receiver |

| Receiver | Status | Notes |
|---|:--:|---|
| Data item (`PIC X`, group, `01`/`77`) | ✅ | The ordinary case |
| Table occurrence, reference modification, qualified name, arithmetic expression | ✅ | Accepted by the evaluator |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | The interpreter accepts a literal receiver, but the parser does not: `::` after a literal is a syntax error. Assign the literal to a data item first |

### 4.2 An expression wherever COBOL-85 allows only an item

COBOL-85 restricts most sending positions to an identifier or a literal.
RustCOBOL evaluates a full expression there instead, which is what removes the
scratch working-storage item the standard forces you to declare.

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. The standard allows only an identifier or literal as the sending field |
| `SET target TO <expression>` | — | — | ● | ✅ | Equivalent to the `COMPUTE` form; the target may be a data item or a control-property lvalue |
| `STRING <expression> … INTO` | — | — | ● | ✅ | A sending item may be an arithmetic expression (`STRING WS-N * 2 …`) or a value-method call (`STRING WS-A::UpperCase() …`); `DELIMITED BY` and the rest stay standard |
| **Type inference** — a `Ctrl::Property` read is a first-class typed value | — | — | ● | ✅ | The numeric/text type flows through the expression, so a property goes straight into arithmetic, a condition or a sending position with **no `PIC` item in between**: `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. A numeric-looking property value is read back as numeric so comparisons and arithmetic stay algebraic rather than character-wise |

## 5. Intrinsic functions

The COBOL-85 intrinsic set arrived with the **1989 amendment** (ANSI
X3.23a-1989); functions added by COBOL 2002 and later are marked in the `20xx`
column. All of the below are implemented.

| Group | Functions | 85 | 20xx | PRC | Status |
|---|---|:--:|:--:|:--:|:--:|
| Length and character | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Length and character (later) | `BYTE-LENGTH`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Case and text | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| Text (later) | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| Numeric conversion | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| Numeric conversion (later) | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| Arithmetic | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| Ordering | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| Statistics | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| Trigonometry and logs | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| Combinatorics | `FACTORIAL` | ● | ○ | — | ✅ |
| Financial | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| Date and time | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. File I/O — organizations and access

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Fixed-length records |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Newline-terminated text; trailing spaces dropped on write |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Built-in, dependency-free ISAM engine |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Own engine (`cobolt-runtime/src/relative.rs`, `PRCREL1` container, disk and MEMORY). `RELATIVE KEY IS` addresses records by integer record number from 1; all three access modes; all seven file verbs dispatch on it. NIST **RL module finished on both axes** — 35/35 compile, 34/34 execution, 354 assertions, 0 failures (engine 1.62.76, module 1.62.77) |
| `RELATIVE KEY IS data-name` (incl. the `KEY`-less spelling) | ● | ○ | — | ✅ | A `RELATIVE data-name` clause with the word `KEY` omitted is the key, not a bare organization clause |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | All three execute |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Ascending on-disk key order |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ | |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ | |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` is COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ | |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Including `GREATER/LESS THAN`, `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ | |
| `FILE STATUS` codes | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Parsed and carried on the statement, **advisory** — there is one run unit, so nothing contends |
| `OPEN … WITH LOCK` (open the file exclusively) | — | ● | — | 🚧 | Same: accepted and advisory in the single-run-unit model |
| `READ … WITH LOCK` | — | ● | — | ✅ | The engine already holds the record under `I-O`; the phrase states the intent |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | Actually releases the lock the engine takes under `I-O` — the one lock phrase with a runtime effect today. `UNLOCK` is in §3 with the other verbs |
| Cross-process file sharing / record-lock enforcement | — | ● | — | ⛔ | Planned; single run-unit model today |

## 7. File I/O — the INDEXED engine (PowerRustCOBOL)

Everything in this section is a platform extension around the standard
`ORGANIZATION IS INDEXED` behaviour above. Detail:
[`indexed-file-format-en.md`](indexed-file-format-en.md),
[`indexed-file-internals-en.md`](indexed-file-internals-en.md),
[`indexed-redb-engine-en.md`](indexed-redb-engine-en.md).

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **The default.** Persistent paged B+tree; records and indexes live in the `ASSIGN` file and are read on demand, so RAM stays bounded on very large files |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | Whole file in RAM, persisted to the `ASSIGN` path on close |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | Dependency-free RLE; crushes the padded runs in typical COBOL records well past 50 % |
| Program-controlled `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Real undo log, memory and disk engines |
| Record locking within a run unit | — | ○ | ● | ✅ | See the cross-process caveat above |
| Selectable engine (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | Also `COBOL_INDEXED_ENGINE`; all behaviour-compatible, `rust` is the default |
| `redb` crash-safe ACID engine | — | — | ● | ✅ | O(1) OPEN (~5 ms at 200 k records), working-set RAM (≥250 M records), survives power loss with no index corruption |
| Self-describing `PRCIDX1` container | — | — | ● | ✅ | Embeds record format + key descriptors; strict open-time validation maps schema mismatch → `39`, missing file → `35`. Not byte-compatible with Fujitsu |
| Per-file transaction log (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt or Grafana/Loki-ready NDJSON — see [`observability-en.md`](observability-en.md) |

## 8. Runtime integrations

Reached from COBOL as runtime `CALL`s and `INVOKE`. None of this is standard
COBOL; it is what makes the language usable for modern applications.

| Capability | 85 | 20xx | PRC | Status | Notes |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | One identical CALL surface for all three; backend chosen from the connection string. Pure-Rust drivers, no system libraries — [`database-runtime-en.md`](database-runtime-en.md) |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | Custom headers |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ | |
| **Charts** — bar / line / pie / area / scatter / donut | — | — | ● | ✅ | Bound to COBOL tables |
| **Text files** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ | |
| **Timers** | — | — | ● | ✅ | |
| **AI agent object hook** | — | — | ● | ✅ | |
| **Rust FFI plugins** | — | — | ● | ✅ | Modules declared under `REPOSITORY`, dispatched via `INVOKE` or direct property mappings |
| **User procedures** | — | — | ● | ✅ | Shared COBOL procedures editable in the IDE, callable as `CALL "PROCEDURE-NAME"` |

## 9. Explicitly out of scope

These will not be implemented. They are listed so the answer is findable rather
than absent.

| Capability | 85 | 20xx | PRC | Status | Why |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, message control / teleprocessing) | ● | — | — | 🚫 | Obsolete in later standards; no modern use |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Superseded by the platform's own reporting and data binding |
| ActiveX / OLE / COM controls | — | — | — | 🚫 | Platform-specific and not portable |

---

## 10. The platform itself

Not COBOL language features — the IDE, compiler and tooling around them. Full
walkthrough in the [developer's guide](developers-guide-en.md).

### 10.1 The IDE

| Capability | Status | Notes |
|---|:--:|---|
| Visual form designer | ✅ | Design canvas with multiple themes (**Liquid Glass**, **Cobalt Steel**), grid snapping, drag-resize of controls and canvas, multi-select alignment, z-ordering |
| Unified rendering engine | ✅ | Pixel-parity between designer, previewer, running application and compiled binary |
| Control catalogue | ✅ | **42 widgets** across Common, Container, Data, Graphics, Menu, Non-visual and Charts |
| Universal corner radius and rounded clipping | ✅ | Nested children clip to a parent's rounded border via corner-notch masking |
| Per-control `Transparency` | ✅ | 0 = opaque … 100 = see-through; fades face, frame and shadow while text, glyphs and border stay legible. Captions below WCAG AA against what is behind them flip to the pole that reads |
| Animator widget | ✅ | Natively renders **GIF / WebP / APNG** |
| Knob, Gauge, Switch, FileDropZone, Maps, Web Search | ✅ | Rotary dial with bipolar fill; radial/linear/donut KPI with automatic warning and critical zones; drag-and-drop or native picker |
| Advanced menu editor | ✅ | Visual tree editor, 122 built-in vector icons, hierarchical nesting, HMAC configuration integrity signatures |
| Data binding and control arrays | ✅ | Direct binding to SQL/data sources; **Visual Repeating Groups** expand GroupBox/Panel arrays from runtime `DataSource` row counts |
| Visual validation and form inspector | ✅ | Real-time error badges for malformed handlers, incomplete bindings, layout anomalies; `rcrun` process manager tracks CPU %, RSS, logs and thread counts live |
| Form Debugger | ✅ | Standalone always-on-top window: breakpoints, step In/Out/Over, variable inspector, animated playback at 1–10 lines/second |
| Agentic AI assistant mesh | ✅ | **rig-core** LLM orchestrator (Ollama, OpenAI, Groq, Alibaba Model Studio, other cloud APIs) running Dev Agent, Editor Assistant and History Compactor, with a live observability log and `↑input ↓output` token readings |
| Grace, the orchestrator | ✅ | Decomposes a request, routes each task to the specialist that owns it, and enforces a one-to-one **Pedantic reviewer** — no specialist approves its own work |
| Chunked Knowledge Base with RAG | ✅ | Indexed one record per subject; ships pre-embedded, GPU with cool-running CPU fallback, **File → Reindex Knowledge Bases** |
| Form lifecycle and windowing | ✅ | One designated **main form** starts an application; per-form chrome and state honoured; `OpenFormSync`/`OpenFormAsync`; window position is a design-time property; per-project entrance and exit effects |
| Multi-window runtime | ✅ | Preview and run screens in dedicated OS viewports (egui multi-viewport) |
| Internationalised UI | ✅ | 6 interface languages: English, Spanish, Portuguese, Japanese, Chinese, French |
| System-font picker | ✅ | Any installed font, rendered in its own typeface, applied live across designer, previews and running forms |
| Non-blocking native file dialogs | ✅ | Open/save/browse without stalling the UI event loop |

### 10.2 The compiler

| Capability | Status | Notes |
|---|:--:|---|
| Single native binary output | ✅ | Serialises the AST with `bincode` + `flate2`, embeds it and all forms via `include_bytes!`, builds with `cargo build --release`, emits one binary in `bin/` — **with no `.cbl` source included** |
| Redistribution notices | ✅ | `bin/` automatically receives `LICENSE`, `NOTICE` and the runtime notice, so distributions carry the required Apache-2.0 notices |
| Real `rustc` diagnostics on a failed build | ✅ | A build failure reports the compiler's own diagnostics, not a summary line |
