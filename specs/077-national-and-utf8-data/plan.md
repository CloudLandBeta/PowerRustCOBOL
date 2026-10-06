<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — National and UTF-8 character data

- **Status:** implemented (1.80.220–1.80.228, 2026-10-06)
- **Spec:** ./spec.md   **Date:** 2026-10-06

## 0. Where we start

Read from the code on 2026-10-06 (worktree `exec-sql`, branch
`features-spec-077`):

- **No national or UTF-8 support anywhere.** `PicKind` has five variants
  (`cobolt-ast/src/data.rs:18`), `Usage` has no `National`, `Literal` has no
  national or UTF-8 form, and the lexer knows only the `X"…"` prefix. `PIC N`
  and `PIC U` silently parse as alphanumeric pictures of *n* bytes
  (`analyze_pic` fallback, `cobolt-parser/src/data.rs:1050`); `USAGE NATIONAL`
  is an "unknown USAGE clause" error.
- **One runtime value model for text:** `CobolValue::String { bytes, capacity }`
  holding UTF-8 bytes, truncated and padded **by bytes**
  (`CobolValue::assign`, `value.rs:524`). A group has no storage of its own:
  its image is its children's `display_bytes` concatenated
  (`environment.rs:2656`), and `set_group_bytes` slices an image back by each
  child's `item_width`. REDEFINES is a copy-on-write overlay
  (`refresh_redefine_peers`) above 256 slots, an alias below.
- **Record files** compute each field's length from `digits + decimals`
  (`files.rs:135`) and build/scatter records through `field_bytes` /
  `distribute`, which go through `as_display_string` (lossy for non-UTF-8).
- **STRING** works on bytes, **UNSTRING** on bytes of lossy text, **INSPECT**
  on bytes except CONVERTING (chars).
- **Comparisons** of non-numerics compare lossy display strings
  (`compare_values`, `interpreter.rs:19222`). `FUNCTION LENGTH` and
  `BYTE-LENGTH` both return byte widths today.
- **Form properties** reach items through `prop_to_value` → `env.set`, and
  items reach properties through `as_display_string` — both text.
- **The IDE's COBOL Structure view computes no item sizes**; it edits text
  blocks. AC15's "30 characters / 60 bytes" needs a small new display there.
- The `.cidx` indexed-file model has its own PICTURE parser
  (`cobolt-indexed/src/pic.rs`).

## 1. Key decisions

### D1 — A national or UTF-8 item holds characters; its bytes are an image

The item's slot keeps its **text** — `CobolValue::String` with the text's
UTF-8 bytes, always exactly *n* characters (national, `PIC U(n)`) or the whole
characters that fit *n* bytes (`PIC U BYTE-LENGTH n`), padded with spaces.
Its **byte image** — UTF-16BE for national (2 × n bytes), UTF-8 padded with
X'20' for UTF-8 (4 × n or n bytes) — is produced only where bytes are
visible: a group's image, REDEFINES, record I/O, `BYTE-LENGTH`, reference
modification of a group.

Why: every text consumer in the runtime (DISPLAY, comparisons, form
properties, string verbs, the debugger) already reads text, so they are right
for national data without a decode at each site, and nothing changes for a
program that uses neither class (R25). The alternative — storing UTF-16
bytes in the slot and decoding at every reader — must find every one of
~40 readers, and each one missed shows raw UTF-16 as text.

The one thing the image model cannot hold is a UTF-16 sequence that is not
text (an unpaired surrogate written by a REDEFINES or read from a file). It
becomes U+FFFD. This is a documented caveat; no conforming program produces
one.

### D2 — The class lives in the environment, registered from the declaration

`CobolEnvironment` gains `char_class: HashMap<String, CharClass>`, keyed by
storage key (occurrence keys resolve through their base item):

```text
CharClass::National { chars }       // PIC N(n): image 2·n bytes
CharClass::Utf8 { chars }           // PIC U(n): image 4·n bytes
CharClass::Utf8Bytes { bytes }      // PIC U BYTE-LENGTH n: image n bytes
```

Every write to a classed key goes through one function, `fit_class(class,
text, justified) -> String`: truncate on a character boundary, pad with
spaces (left-pad when JUSTIFIED RIGHT). Every image goes through
`class_image(class, text) -> Vec<u8>` and back through `class_text(class,
bytes) -> String`. The three widths come from one `class_width(class)`.

### D3 — AST additions, at the end (bincode)

- `PicKind::National`, `PicKind::Utf8` (appended).
- `Usage::National`, `Usage::Utf8` (appended).
- `Literal::National(String)`, `Literal::Utf8(String)` (appended).
- `DataDecl.byte_length: Option<u32>` (appended) for `BYTE-LENGTH n`.
- `InitCategory::National`, `InitCategory::Utf8` (appended).

`PicClause.digits` keeps the character count for N and U pictures, as it does
for X.

### D4 — Lexer and parser

- New literal tokens: `N"…"`/`N'…'`, `NX"…"`, `U"…"`/`U'…'`, `UX"…"`. `NX`
  must be a multiple of four hex digits and `UX` must be well-formed UTF-8
  (IBM's table): otherwise `Token::Error` with the reason, reported on its
  line (R16, AC3, AC13). `U` literals accept IBM's escapes `\uhhhh`,
  `\U00hhhhhh` and `\\` (surrogates refused).
- `PIC N(n)`, `PIC U(n)` → `PicKind::National` / `PicKind::Utf8`; `PIC U`
  with `BYTE-LENGTH n`. `USAGE [IS] NATIONAL` / `UTF-8` alone or with a
  picture; `USAGE NATIONAL` with `PIC 9…` or an edited picture is Q3's
  "not supported yet" error.

### D5 — Semantics checked before run (cobolt-semantic)

- A national or UTF-8 item as an arithmetic operand or receiver: error (R16).
- National numeric / national-edited: "not supported yet" error (Q3).
- `NATIONAL-OF` / `DISPLAY-OF` with a literal code page that is not one of
  UTF-8 / 1208, WINDOWS-1252 / 1252, ISO-8859-1 / 819: error (AC4).
- A national or UTF-8 VALUE literal longer than the item: error.

### D6 — Moves and figuratives

Moves into a classed receiver take the sender's text (alphanumeric data is
UTF-8 in this runtime) and `fit_class` it. Moves out of a classed sender give
its text to the receiver, and an alphanumeric receiver truncates **on a
character boundary** (R8, AC5) — a classed sender is the only case where the
alphanumeric path changes, so no existing program moves differently. Group
moves carry images (R12).

Figuratives in a classed context: SPACE → U+0020, ZERO → "0", QUOTE → `"`,
LOW-VALUE → U+0000, HIGH-VALUE → U+FFFF (national) / U+10FFFF (UTF-8, IBM's
UX'F48FBFBF'), ALL x → x repeated by character.

### D7 — Comparisons (R9, R19, Q4)

When either operand is classed or a national/UTF-8 literal, the operands are
compared as text: the shorter padded with spaces **in characters**, then
compared **by code point** (UTF-8 byte order is code-point order). No
COLLATING SEQUENCE applies, as IBM documents for UTF-8.

### D8 — Lengths and functions

- `LENGTH`: characters (n). `BYTE-LENGTH`: image bytes (2n, 4n, n).
- `NATIONAL-OF(a [, cp])` decodes `a`'s bytes from `cp` (default UTF-8) into
  national text; `DISPLAY-OF(n [, cp])` encodes text into `cp` bytes
  (default UTF-8). Code pages by name or CCSID: UTF-8/1208, 1252, 819.
  Unconvertible characters become `?`-substitutes as IBM does (X'7F' for a
  single-byte target, U+FFFD from bytes that are not valid in `cp`).
- `ULENGTH`, `UPOS`, `USUBSTR`, `UVALID`, `UWIDTH`, `USUPPLEMENTARY` exactly
  as Q6 records, over the argument's bytes: UTF-8 for alphanumeric and UTF-8
  operands, the UTF-16 image for national operands.

### D9 — String verbs (R10)

When any operand of STRING, UNSTRING or INSPECT is classed (or a national /
UTF-8 literal), the verb runs on characters: positions (POINTER,
ref-mod), counts (COUNT IN, TALLYING) and widths are in characters, and the
results are stored through `fit_class`. Without a classed operand, today's
byte paths run unchanged.

### D10 — Files (R14, R22)

- `RecordLayout` takes a field's length from `class_width` for classed
  fields, and `field_bytes` / `distribute` go through `class_image` /
  `class_text`: national fields are UTF-16BE in the record, UTF-8 fields their
  padded bytes. Keys over national fields are their UTF-16BE bytes.
- LINE SEQUENTIAL writes and reads a national field as UTF-8 text of *n*
  characters (R14): the line is built and split field by field in
  characters, not by image offsets.
- The `.cidx` PICTURE parser accepts N and U with the same widths, so an
  indexed file defined in the IDE can carry national fields.

### D11 — ACCEPT (R11, Q7)

`ACCEPT` into a classed item decodes the typed bytes as UTF-8 and, when they
are not valid UTF-8, as Windows-1252. Other receivers keep today's bytes.

### D12 — Hosts, forms, debugger, IDE

- Forms (R15): no new path — `prop_to_value` gives text and `env.set` fits it;
  item → property reads text. Proved by a form test in each host (R26).
- Debugger: a classed item shows as category `national` / `utf-8`, its value
  as text, its length in image bytes.
- IDE: syntax colouring of the literal prefixes and the new words;
  IntelliSense entries for the functions and clauses; the COBOL Structure
  view gains an item-size line for the item under the caret
  ("`WS-MSG` — 30 characters, 60 bytes", `Tr` ×6).

## 2. Files

| Crate | Files |
|---|---|
| cobolt-ast | `data.rs`, `expr.rs`, `stmt.rs` |
| cobolt-lexer | `token.rs`, `lexer.rs`, `keywords.rs` |
| cobolt-parser | `data.rs`, `expr.rs`, `stmt.rs` |
| cobolt-semantic | `type_checker.rs`, `symbol_table.rs`, a new `national.rs` check |
| cobolt-runtime | `value.rs`, `environment.rs`, new `national.rs` (classes, fit, images, code pages, U-functions), `interpreter.rs` (moves, compare, functions, string verbs, ACCEPT, INITIALIZE, debugger rows), `files.rs` |
| cobolt-indexed | `pic.rs` |
| cobolt-ide | `panels/editor.rs` (colours, IntelliSense), `panels/cobol_structure.rs` (sizes), `panels/debugger.rs`, `i18n.rs` |
| cobolt-compiler | System KB doc tables |
| docs | `developers-guide-en.md`, `cobol85-supported-syntax-en.md` |
| tests | `tests/cobol/national/*.cbl`, `tests/cobol/utf8/*.cbl`, runtime/parser/semantic/IDE unit tests |

## 3. Risks

- **R25 regression.** Every new path is entered only for a classed key or a
  national/UTF-8 literal. Verified by the whole sweep and the NIST modules
  after each milestone.
- **The image/text split misses a byte reader** (a group image built
  elsewhere). Mitigation: one `display_bytes` entry point for leaves, and an
  AC2 test that REDEFINES and group-moves national data.
- **Undefined IBM corners** (`USUBSTR` out of range) are given a defined,
  documented, non-stopping result.

## 4. Milestones

| M | Delivers | Proof |
|---|---|---|
| M1 | AST, lexer, parser, semantic checks (Phase 1 + 2 syntax) | parser/lexer/semantic tests; AC3, AC11, AC13 Check halves |
| M2 | Runtime classes: storage, moves, figuratives, comparisons, LENGTH/BYTE-LENGTH, DISPLAY, ACCEPT, INITIALIZE, group/REDEFINES images | AC1, AC2, AC5, AC6, AC8, AC12 |
| M3 | NATIONAL-OF / DISPLAY-OF and the U-functions | AC4, AC14 |
| M4 | STRING / UNSTRING / INSPECT on characters | AC7 |
| M5 | Files (record, line sequential, indexed keys, `.cidx`) | AC9 |
| M6 | Forms and the four hosts | AC10, AC17 |
| M7 | IDE: colours, IntelliSense, Structure view sizes, debugger | AC15 |
| M8 | Docs, KB, test programs' result blocks, full sweep, NIST | AC16, AC18 |
