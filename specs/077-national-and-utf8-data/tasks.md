<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — National and UTF-8 character data

- **Status:** in progress (implementation authorised by the operator,
  2026-10-06)
- **Plan:** ./plan.md   **Date:** 2026-10-06

Each milestone ends with a `z` bump, a CHANGELOG entry and a commit on
`features-spec-077`. Every task's tests print what they exercised.

## T0 — Baseline

- [x] **T0** Record the sweep and the NIST figures this work starts from
  (the 087 T39 sweep, 2026-10-06: lexer 172, ast 32, parser 184, semantic 98,
  runtime 1152, …; NIST strict 420/420, NC 4614, SQ 624, IX 574, RL 354,
  IC 309), and which of the spec's eight protected modules (NC, SQ, IF, IX,
  ST, RL, IC, SM) the harness runs.
  - Result: the harness runs all eight. IF 841, ST 735, SM 311 assertions,
    100 % — with NC 4614, SQ 624, IX 574, RL 354, IC 309 that is 8,362 of
    8,362, the spec's figure.

## M1 — Syntax and Check

- [x] **T1 — AST** (D3): `PicKind::{National, Utf8}`, `Usage::{National,
  Utf8}`, `Literal::{National, Utf8}`, `DataDecl.byte_length`,
  `InitCategory::{National, Utf8}` — all appended. Every exhaustive match in
  the workspace handles them. *Verify:* `cargo build` of the workspace.
- [x] **T2 — Lexer** (D4): `N"…"`, `N'…'`, `NX"…"`, `U"…"`, `U'…'`, `UX"…"`
  tokens; NX/UX validation and U escapes, as `Token::Error` with the
  reason. *Verify:* lexer unit tests (each form, each error, `\u`/`\U`).
- [x] **T3 — Parser** (D4): `PIC N`, `PIC U`, `BYTE-LENGTH n`, `USAGE
  NATIONAL` / `UTF-8` (with and without a picture), national/UTF-8 VALUE
  literals, `INITIALIZE … REPLACING NATIONAL / UTF-8`. *Verify:* parser
  tests: the AST each declaration produces.
- [x] **T4 — Check** (D5): arithmetic on national/UTF-8; national numeric
  and national-edited (Q3); literal code pages; VALUE too long. *Verify:*
  semantic tests, each error on its line (AC3, AC11, AC13 Check halves).
- [x] **T5 — M1 wrap-up:** `z` bump, CHANGELOG, commit.

## M2 — The runtime classes

- [x] **T6 — `national.rs`** (D1, D2): `CharClass`, `fit_class`,
  `class_image`, `class_text`, `class_width`, UTF-16BE codec. *Verify:* unit
  tests (whole-character truncation, padding, JUSTIFIED, round trips,
  surrogate pairs).
- [x] **T7 — Environment** (D2): register classes from declarations
  (OCCURS included); VALUE through `fit_class`; every write to a classed key
  through `fit_class`; `display_bytes` / `item_width` / `pic_storage_len` /
  `declared_width` / `set_group_bytes` through the image functions; REDEFINES
  both ways. *Verify:* AC1, AC2, AC12 as runtime tests.
- [x] **T8 — Moves and figuratives** (D6): classed receiver, classed
  sender into alphanumeric (character boundary), group moves, figuratives,
  JUSTIFIED. *Verify:* AC5 and a figurative table test.
- [x] **T9 — Comparisons** (D7). *Verify:* AC6, plus padding and ordering
  cases.
- [x] **T10 — LENGTH / BYTE-LENGTH, DISPLAY, ACCEPT (Q7), INITIALIZE**
  (D8, D11). *Verify:* AC1, AC8, AC12; INITIALIZE and REPLACING NATIONAL.
- [x] **T11 — Debugger rows** (D12). *Verify:* runtime test on `debug_row`.
- [x] **T12 — M2 wrap-up:** sweep + NIST, `z` bump, CHANGELOG, commit.
  - Result (1.80.221): every crate green except the known
    `every_document_ships_in_every_language`; NIST strict unchanged, and NC,
    SQ, IF, IX, ST, RL, IC and SM all 100 % — 8,362 of 8,362 assertions.
  - Found on the way, outside this spec: `X"…"` literals hold one
    *character* per hex pair, not one byte, so `X"41C3"` cannot spell
    ill-formed UTF-8. Flagged as its own fix; AC14's `UVALID` half is tested
    through a REDEFINES over national data until then.

## M3 — Functions

- [x] **T13 — NATIONAL-OF / DISPLAY-OF** (D8). *Verify:* AC4.
- [x] **T14 — The U-functions** (D8, Q6). *Verify:* AC14 and IBM's own
  examples ('Käfer', x'61CC88', x'6162D0B0E4BA8CF5646364' → 8, …).
- [x] **T15 — M3 wrap-up.**
  - Result (1.80.222): IBM's own examples pass unchanged — Käfer (UPOS
    1 2 4 5 6, UWIDTH 1 2 1 1 1, USUBSTR Kä / äf / fe), the national
    nx'0054…0073' (ULENGTH 7, UPOS … 15, UWIDTH … 4 2), the G-clef
    (USUPPLEMENTARY 3, both classes) and both UVALID national examples (2
    and 4). UPPER-CASE / LOWER-CASE of national or UTF-8 data follow
    Unicode, as IBM documents; alphanumeric keeps the ASCII rule.

## M4 — String verbs

- [x] **T16 — STRING, UNSTRING, INSPECT on characters** (D9). *Verify:*
  AC7, POINTER / COUNT IN / TALLYING in characters; byte paths unchanged.
- [x] **T17 — M4 wrap-up.**
  - Result (1.80.223): INSPECT and UNSTRING run one scan over code units —
    bytes as before, characters for national / UTF-8 data — so byte
    positions are unchanged: sweep green (known docs test aside), NIST
    8,362 of 8,362. Found on the way: an `N"…"` / `U"…"` literal could not
    open an operand (DISPLAY list, STRING sender, abbreviated condition);
    fixed in the parser with a test.

## M5 — Files

- [x] **T18 — Record layout and I/O** (D10): widths, images, keys; LINE
  SEQUENTIAL as text. *Verify:* AC9 (indexed, reopened by a second run),
  a sequential and a line-sequential round trip.
- [x] **T19 — `.cidx`** PICTURE N and U. *Verify:* cobolt-indexed tests.
- [x] **T20 — M5 wrap-up.**
  - Result (1.80.225): AC9 passes — a `PIC N(20)` key and a `PIC N(40)`
    field written, read by key, rewritten, and reopened by a second run in
    key order with every character intact; the record is 120 bytes. Record
    SEQUENTIAL and LINE SEQUENTIAL round trips pass, the latter as readable
    UTF-8 text. Sweep green (known docs test aside); NIST 8,362 of 8,362.

## M6 — Forms and hosts

- [ ] **T21 — Form properties** (R15) and parity (R26). *Verify:* AC10 in
  Run Form and a compiled binary; AC17 for the AC1–AC14 programs under
  `rcrun run` and a compiled binary.
- [ ] **T22 — M6 wrap-up.**

## M7 — IDE

- [ ] **T23 — Colours and IntelliSense** (R23). *Verify:* editor tests.
- [ ] **T24 — Structure view sizes and debugger** (R23, `Tr` ×6). *Verify:*
  AC15 test; i18n tests.
- [ ] **T25 — M7 wrap-up.**

## M8 — Documentation and acceptance

- [ ] **T26 — Test programs** `tests/cobol/national/`, `tests/cobol/utf8/`
  with GOLDEN RULE #7 result blocks (AC18).
- [ ] **T27 — Guide, syntax reference, System KB** (R24); regenerate
  `chunked.data`.
- [ ] **T28 — Full sweep and NIST** (AC16); tick the ACs; final `z` bump.
