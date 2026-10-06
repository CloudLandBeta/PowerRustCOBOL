<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — National and UTF-8 character data (PIC N / USAGE NATIONAL, PIC U / USAGE UTF-8)

- **Status:** clarified (operator answered Q1, Q3, Q4, Q7 on 2026-10-06; Q2
  takes its suggestion; Q5 and Q6 settled from IBM's documentation the same
  day); implementation authorised by the operator, 2026-10-06
- **Folder:** specs/077-national-and-utf8-data/
- **Author:** Claude (for the operator)   **Date:** 2026-09-28

## 1. Overview

RustCOBOL holds alphanumeric text as UTF-8 bytes, and `PIC X(n)` counts bytes.
That is the standard's rule for alphanumeric data, but UTF-8 spends two bytes
on `ç`, `ã` or `é` and three on `–` or `€`. So a field sized in characters
overflows as soon as the text is accented: `PIC X(30) VALUE "Configuração
concluída – ok"` has 27 characters and 32 bytes, and it keeps only what fits
in 30. Programs brought over from Windows now read correctly, UTF-8 or
Windows-1252 (1.70.333 and 1.70.334), and they carry exactly this kind of
text.

COBOL answers this in two ways, and this spec adds both:

- **Phase 1, standard (COBOL 2002):** the **national** class. `PIC N(n)` /
  `USAGE NATIONAL` holds *n* characters, each stored in UTF-16 (two bytes).
  It comes with national literals (`N"…"`, `NX"…"`), the conversion functions
  `NATIONAL-OF` and `DISPLAY-OF`, and the implicit conversions between
  national and alphanumeric data.
- **Phase 2, IBM Enterprise COBOL's extension:** the **UTF-8** class.
  `PIC U(n)` / `USAGE UTF-8` holds *n* characters stored as UTF-8. It comes
  with UTF-8 literals and IBM's Unicode functions for UTF-8 text: `ULENGTH`,
  `UPOS`, `USUBSTR`, `UVALID`, `UWIDTH` and `USUPPLEMENTARY`.

Both phases share one conversion core: the runtime's text is UTF-8, national
is UTF-16, and every crossing between the classes goes through the same
rules.

## 2. Goals / Non-goals

- **Goals:**
  - A field that holds *n* characters whatever the language, in the form a
    COBOL developer already knows from IBM or Micro Focus: `PIC N`, and
    `PIC U` for UTF-8.
  - The same record layout as those compilers: `PIC N(10)` is 20 bytes of
    UTF-16 in a file record, a `REDEFINES` or a group move.
  - Mixing classes works the way the standard says: `MOVE`, comparison,
    `STRING`, `UNSTRING`, `INSPECT`, `DISPLAY`, `ACCEPT`, `INITIALIZE`.
  - Form controls, whose properties are text, move to and from national and
    UTF-8 items like any other item.
  - Each phase ships, and is tested, on its own.
- **Non-goals:**
  - Changing `PIC X`. It keeps counting bytes, as the standard says, and
    every existing program behaves as it does today.
  - National numeric and national-edited items (`PIC 9 USAGE NATIONAL`,
    national-edited pictures). See Q3.
  - IBM's variable-length UTF-8 items (`DYNAMIC LENGTH`) and anything beyond
    fixed character-length and fixed byte-length `PIC U`.
  - `LOCALE` in `SPECIAL-NAMES` and the locale-sensitive functions
    (`LOCALE-DATE`, `LOCALE-TIME`, locale collation). They are separate
    COBOL 2002 features.
  - Source-code encodings beyond UTF-8 and Windows-1252. Those two are
    already read (1.70.333 and 1.70.334).

## 3. User stories

- As a COBOL developer writing Portuguese, Spanish or French text, I want a
  field declared for 30 characters to hold 30 characters, so that accented
  names and messages are not cut short.
- As a developer porting from IBM Enterprise COBOL or Micro Focus, I want
  `PIC N`, `N"…"`, `NATIONAL-OF` and `DISPLAY-OF` to behave as I know them,
  and national fields in records to have the same byte layout, so that my
  programs and data files carry over.
- As a developer porting IBM code that uses UTF-8 (`PIC U`, `ULENGTH`,
  `USUBSTR`…), I want those to compile and run the same way.
- As a form developer, I want to move a TextBox's `Text` into a `PIC N` or
  `PIC U` field and back, so that what the operator typed keeps every
  character.
- As a developer who hits a mistake — a national item in arithmetic, a
  national literal that is not valid hex — I want a clear diagnostic when the
  program is checked, not a wrong value at run time.

## 4. Requirements (EARS)

### Phase 1 — National (COBOL 2002)

- **R1 (ubiquitous):** The system shall accept `PIC N(n)` (and `N` repeated),
  and `PIC N(n) USAGE [IS] NATIONAL` / `USAGE NATIONAL` on an elementary item,
  as a **national** item of *n* character positions.
- **R2 (ubiquitous):** A national item shall occupy **2 bytes per character
  position**, stored as **UTF-16**, and shall contribute that byte length to
  its record, group and `REDEFINES` layout. The byte order is settled in Q1.
- **R3 (ubiquitous):** The system shall accept national literals:
  - `N"…"` and `N'…'`, whose text is the source's characters;
  - `NX"…"`, whose hex digits are UTF-16 code units, four per character.
- **R4 (event):** When a national item receives text longer than its
  character positions, the system shall keep the leftmost characters, whole.
  When it receives shorter text, the system shall pad on the right with
  national spaces (U+0020). `JUSTIFIED RIGHT` shall reverse both, as it does
  for alphanumeric items.
- **R5 (ubiquitous):** Figurative constants shall have national meanings in a
  national context: `SPACE`, `ZERO`, `QUOTE`, `HIGH-VALUE`, `LOW-VALUE` and
  `ALL "x"`.
- **R6 (ubiquitous):** The system shall provide:
  - `FUNCTION NATIONAL-OF(alphanumeric [, code-page])`, which returns
    national text;
  - `FUNCTION DISPLAY-OF(national [, code-page])`, which returns
    alphanumeric text.

  Without a code page, the alphanumeric side is UTF-8. With one, at least
  `UTF-8`, `1252` / `WINDOWS-1252` and `ISO-8859-1` shall be accepted. The
  exact names are settled in Q2.
- **R7 (ubiquitous):** For a national operand, `FUNCTION LENGTH` shall return
  its character positions and `FUNCTION BYTE-LENGTH` its bytes. For
  `PIC N(30)`, that is 30 and 60.
- **R8 (event):** When a `MOVE` has a national sending item and an
  alphanumeric receiver, or the reverse, the system shall convert the text
  between UTF-16 and the runtime's UTF-8.
  - Truncation shall fall on a **character boundary**: no half character is
    ever stored.
  - Any padding shall be spaces of the receiver's class.
- **R9 (event):** When a comparison has a national operand, the system shall
  compare as national: the other operand is converted, the shorter is padded
  with national spaces, and characters compare in code-point order unless
  Q4 settles another collation.
- **R10 (ubiquitous):** `STRING`, `UNSTRING` and `INSPECT` shall accept
  national operands, count and move in character positions, and convert as
  R8 wherever the classes are mixed. The standard's restrictions on mixing
  shall be diagnosed, not guessed at.
- **R11 (event):** When a national item is `DISPLAY`ed, the system shall
  write its characters as UTF-8. When it is the target of an `ACCEPT`, the
  system shall store the characters typed.
- **R12 (ubiquitous):** A group item containing national items shall remain
  an alphanumeric group. A group move shall carry the national items' UTF-16
  bytes as they stand, as a group move does for every child.
- **R13 (ubiquitous):** `INITIALIZE` shall set national items to national
  spaces, and `INITIALIZE … REPLACING NATIONAL …` shall be accepted.
- **R14 (ubiquitous):** A national item shall be accepted as a field of a
  `SEQUENTIAL`, `LINE SEQUENTIAL`, `RELATIVE` or `INDEXED` record, and as all
  or part of a record key.
  - In record files, the UTF-16 bytes shall be written as they stand.
  - In a `LINE SEQUENTIAL` file, a national item's text shall be written and
    read as UTF-8, the way the file's text is.
- **R15 (event):** When a form control's property (a TextBox's `Text`, a
  Label's `Caption`…) is moved to a national item, or a national item is
  moved to such a property, the system shall convert as R8. No character is
  lost within the item's length.
- **R16 (constraint):** The system shall not accept a national item as an
  operand of arithmetic. It shall not accept an `NX` literal whose digit
  count is not a multiple of four, or that is not valid hexadecimal. Both
  are diagnosed when the program is checked, with the line.

### Phase 2 — UTF-8 (IBM Enterprise COBOL extension)

- **R17 (ubiquitous):** The system shall accept `PIC U(n)` and
  `USAGE [IS] UTF-8` on an elementary item, as a **UTF-8 item** of *n*
  characters.
  - It is held as UTF-8, and its record byte length is **4 × n**, the most
    *n* UTF-8 characters can take. The value is padded with spaces to that
    length, as IBM lays it out; Q5 confirms this.
  - `PIC U BYTE-LENGTH n` (fixed byte-length) shall be accepted: the item
    holds as many whole characters as fit in *n* bytes.
- **R18 (ubiquitous):** The system shall accept UTF-8 literals: `U"…"` and
  `U'…'`, whose text is the source's characters, and `UX"…"`, whose hex
  digits are UTF-8 bytes that must form valid UTF-8.
- **R19 (event):** When a UTF-8 item receives text, the system shall keep at
  most *n* characters, or the whole characters that fit its byte length,
  never a partial one.
  - Moves between UTF-8, national and alphanumeric items shall convert as R8.
  - Comparisons with a UTF-8 operand shall compare as national.
- **R20 (ubiquitous):** For a UTF-8 operand, `FUNCTION LENGTH` shall return
  its character count and `FUNCTION BYTE-LENGTH` its storage bytes.
- **R21 (ubiquitous):** The system shall provide IBM's Unicode functions over
  UTF-8 text, whether it is held in a UTF-8 item or in alphanumeric data:
  - `ULENGTH`: the number of characters;
  - `UPOS`: the byte position where a given character begins;
  - `USUBSTR`: a substring by character position and character count;
  - `UVALID`: 0 when the text is valid UTF-8, otherwise the position of the
    first invalid byte;
  - `UWIDTH`: the byte width of the character at a position;
  - `USUPPLEMENTARY`: the position of the first character beyond the Basic
    Multilingual Plane, or 0.

  Each shall behave as IBM documents it. Q6 lists the details to confirm.
- **R22 (ubiquitous):** R8–R16 shall apply to UTF-8 items as they apply to
  national items. The exceptions:
  - a group move carries the UTF-8 item's bytes;
  - in a `LINE SEQUENTIAL` file, its text is its bytes.

### Both phases

- **R23 (ubiquitous):** The IDE shall recognise the new pictures, `USAGE`
  clauses, literals and functions:
  - in syntax colouring;
  - in IntelliSense;
  - in the COBOL Structure view, which shows an item's size in characters
    and in bytes;
  - in the debugger, which shows a national or UTF-8 item's value as
    readable text, with its byte length.
- **R24 (ubiquitous):** The System KB documentation and the Developer's Guide
  shall describe both classes, the conversions, the functions and the
  caveats, with COBOL examples. The chunked KB store shall be regenerated in
  the same change.
- **R25 (constraint):** The system shall not change the behaviour of any
  program that uses neither class. `PIC X` keeps counting bytes, and every
  protected NIST module keeps 100 % of its assertions.
- **R26 (ubiquitous):** Every form of the feature shall work the same under
  `rcrun run`, Run Form, embedded child forms and a compiled binary.

## 5. Acceptance criteria

- [ ] **AC1 (R1, R2, R4, R7):** `01 WS-MSG PIC N(30) VALUE N"Configuração
  concluída – ok".`
  - `DISPLAY WS-MSG` shows all 27 characters followed by 3 spaces.
  - `FUNCTION LENGTH(WS-MSG)` is 30, and `FUNCTION BYTE-LENGTH(WS-MSG)` is
    60.
- [ ] **AC2 (R2, R12):** A group made of `PIC N(3)` and `PIC X(2)` has a byte
  length of 8.
  - A `REDEFINES` of it as `PIC X(8)` shows the UTF-16 bytes of the three
    characters.
  - A group move to an identical group restores the same characters.
- [ ] **AC3 (R3, R16):** `N"Ação"`, `N'Ação'` and `NX"00410063"` compile and
  hold the expected characters. `NX"0041006"` (7 digits) is rejected when
  the program is checked, with its line.
- [ ] **AC4 (R6):**
  - `DISPLAY-OF(NATIONAL-OF("Ação"))` returns `Ação`.
  - `NATIONAL-OF(x, "1252")`, where `x` holds Windows-1252 bytes for
    `Ação`, returns `Ação`.
  - An unknown code page is a diagnosed error.
- [ ] **AC5 (R8):** `MOVE` from a `PIC N(10)` holding `Configuração` to a
  `PIC X(10)` stores whole characters only (no broken UTF-8), padded with
  spaces. The reverse move gives the characters back.
- [ ] **AC6 (R9):** `IF WS-N = "Ação"` is true for a `PIC N(10)` holding
  `Ação`. Ordering follows code points; this AC is reworded if Q4 decides
  otherwise.
- [ ] **AC7 (R10):**
  - `INSPECT` a `PIC N` item, counting `N"ç"`, returns a character count.
  - `STRING` of two national items into a national receiver, and
    `UNSTRING` of a national item by `N","`, give the expected pieces.
- [ ] **AC8 (R11):** `ACCEPT` into a `PIC N(20)` of the typed text `Olá,
  João` stores those 9 characters (this read "10" until 2026-10-06; the text has nine). `DISPLAY` shows them back.
- [ ] **AC9 (R14):** An indexed file whose record holds a `PIC N(20)` key
  and a `PIC N(40)` field is written, read back by key and rewritten. It is
  reopened by a second run, and every character survives. The record's
  byte length is the declared one.
- [ ] **AC10 (R15):** A form moves a TextBox's `Text` of `Configuração – ok`
  into `PIC N(30)` and back into a Label's `Caption`. The caption shows the
  same text, under Run Form and in a compiled binary.
- [ ] **AC11 (R16):** `ADD 1 TO WS-N` on a national item is rejected when
  the program is checked.
- [ ] **AC12 (R17, R19, R20):** `01 WS-U PIC U(5) VALUE U"Ação!".`
  - `ULENGTH`, and `LENGTH` on the item, are both 5.
  - `BYTE-LENGTH` is 20, with the value padded to its storage.
  - `MOVE U"Configuração" TO WS-U` keeps `Confi`, whole characters.
- [ ] **AC13 (R18):** `UX"C3A7"` is `ç`. `UX"C3"`, which is not valid UTF-8,
  is rejected when the program is checked.
- [ ] **AC14 (R21):** Over the alphanumeric text `"Aç€😀"`, each function
  returns the value IBM documents:
  - `ULENGTH` = 4 and `UPOS(…, 3)` = 4;
  - `USUBSTR(…, 2, 2)` = `ç€` and `UWIDTH(…, 4)` = 4;
  - `USUPPLEMENTARY` = 7 (the byte where `😀` begins — IBM returns a byte
    position; this read 4 before Q6 was settled) and `UVALID` = 0;
  - over `X"41C3"`, `UVALID` returns the position of the bad byte.
    *(2026-10-06: this runtime reads each `X"…"` pair as a character, not a
    byte — a defect older than this spec, flagged as its own fix — so
    `X"41C3"` cannot spell ill-formed UTF-8 yet. The same check runs over
    X'00E9', a `REDEFINES` of `N"é"`, which reports 2.)*
- [ ] **AC15 (R23):** In the IDE, `PIC N`, `PIC U`, `USAGE NATIONAL`,
  `USAGE UTF-8`, `N"…"`, `NX"…"`, `U"…"`, `UX"…"` and the new functions are
  coloured and offered by IntelliSense. The COBOL Structure view shows
  `PIC N(30)` as 30 characters / 60 bytes.
- [ ] **AC16 (R25):**
  - Every existing test passes unchanged.
  - The eight protected NIST modules (NC, SQ, IF, IX, ST, RL, IC, SM) stay
    at 100 %: 8,362 of 8,362 assertions.
- [ ] **AC17 (R26):** The test programs of AC1–AC14 give the same results
  under `rcrun run` and as a compiled binary.
- [ ] **AC18 (GOLDEN RULE #7):** The test programs in
  `tests/cobol/national/` and `tests/cobol/utf8/` each print one closing
  summary. It lists every case exercised (the pictures, literals, moves and
  functions, by name) and the pass/fail tally, and times any bulk work, such
  as the indexed-file round trip of AC9.

## 6. Constraints & steering check

- **Classification:** **feature**. Both classes are beyond the COBOL-85 the
  product targets: the national class is COBOL 2002, and UTF-8 is an IBM
  extension. It goes on the `features` branch, is announced (when the
  operator asks) on forum f=96, and bumps `z` only.
- **i18n (6 languages):** no new user-facing strings are expected; the
  pictures, clauses and functions are COBOL and stay English. If the
  debugger or the Structure view gains a label such as "characters /
  bytes", it is a `Tr` field in all six languages.
- **Generated code:** codegen writes no national or UTF-8 items today, so
  the generated banner and the regenerate contract are untouched. Controls
  exchange text, and R15 covers them.
- **Docs:**
  - `docs/developers-guide-en.md` gets a section on both classes.
  - `docs/cobol85-supported-syntax-en.md` lists them as extensions beyond
    COBOL-85.
  - The System KB tables are updated and `chunked.data` regenerated (R24).
  - Translations follow GOLDEN RULE #8: they are deleted, not patched.
- **Parity:** `interpreter-binary-parity` applies. The runtime changes reach
  `rcrun run-form`, embedded child forms and the compiled binary (R26).
- **Regression:** protected NIST modules at 100 % on both axes (R25, AC16).
- **Tests:** user-provided tests are report-or-fix; new COBOL tests follow
  GOLDEN RULE #7 (AC18).

## 7. Open questions

- **Q1 — UTF-16 byte order.** **Settled (operator, 2026-10-06): big-endian.**
  Big-endian, as IBM Enterprise COBOL on z/OS
  stores it, or little-endian, as Micro Focus does on x86? This decides
  whether data files move byte-for-byte between us and the one or the other.
  *Suggested:* big-endian, as a default a program cannot see, since it only
  shows in raw bytes (`REDEFINES`, group moves, files).
- **Q2 — Code-page names.** **Settled (2026-10-06): the suggestion stands** —
  names and CCSIDs for UTF-8, Windows-1252 and ISO-8859-1. Which spellings `NATIONAL-OF` / `DISPLAY-OF`
  accept, and whether a numeric CCSID (IBM's `1208` for UTF-8, `1252`,
  `819`) counts as a code page. *Suggested:* accept both the names and the
  CCSIDs for UTF-8, Windows-1252 and ISO-8859-1.
- **Q3 — National numeric and national-edited.** **Settled (operator,
  2026-10-06): a later spec**, once `PIC N` and `PIC U` have shipped; until
  then Check reports them as not supported yet. Out of scope here.
- **Q4 — Collation.** **Settled (operator, 2026-10-06): code-point order**;
  accent-aware collation belongs to a future `LOCALE` spec. Code-point order, the standard default with no
  `COLLATING SEQUENCE`, or something aware of accented letters (Portuguese
  `á` beside `a`)? *Suggested:* code points now, and accent-aware collation
  with the `LOCALE` spec, if it comes.
- **Q5 — `PIC U` storage and padding.** **Settled (2026-10-06, from IBM
  Enterprise COBOL 6.4, Language Reference and Programming Guide):** a fixed
  character-length `PIC U(n)` reserves 4 × n bytes; every unused byte is a
  UTF-8 space (X'20'); a move truncates at a character boundary, keeping at
  most n characters. `PIC U BYTE-LENGTH n` reserves exactly n bytes, holds the
  whole characters that fit, and is padded with X'20' to n bytes. `LENGTH` of
  `PIC U(n)` is n; `BYTE-LENGTH` is the storage in bytes (4 × n). Confirm IBM's layout for a fixed
  character-length item: the 4 × n bytes, and how the unused bytes are
  filled. Confirm how a `BYTE-LENGTH` item is padded.
- **Q6 — The U-functions' edge cases.** **Settled (2026-10-06, same
  sources):** none takes a byte/character selector. `UPOS` returns the BYTE
  position where the n-th character starts and `UWIDTH` its width in bytes;
  both return 0 for n ≤ 0 or n > `ULENGTH`. `USUBSTR` counts characters.
  `UVALID` returns 0, or the byte position of the first byte of the first
  ill-formed sequence (a sequence cut off at the end is reported at its lead
  byte); for a national argument, the position in UTF-16 units.
  `USUPPLEMENTARY` returns the BYTE position of the first character above
  U+FFFF (UTF-16 units for national), or 0 — so AC14's expected value is 7,
  not 4 (corrected below). IBM leaves `USUBSTR` out of range undefined; here
  it returns the characters that exist and never stops the program. Confirm against IBM's behaviour, in
  our own tests and wording:
  - positions that are 1-based and count bytes (`UPOS`, `UWIDTH`) versus
    characters (`USUBSTR`);
  - the results for an out-of-range argument;
  - what `UVALID` reports for a truncated sequence at the end.
- **Q7 — `ACCEPT` from a terminal whose input is not UTF-8.** **Settled
  (operator, 2026-10-06): UTF-8, with Windows-1252 as the fallback.** Treat it as
  UTF-8, or decode Windows-1252 as the source reader does?
  *Suggested:* UTF-8, with Windows-1252 as the fallback, as for sources.
