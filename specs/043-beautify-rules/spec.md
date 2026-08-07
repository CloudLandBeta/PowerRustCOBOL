<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 043 — Beautify rules (operator-dictated, 2026-08-07)

Applies to **every editor surface that offers Beautify**: the main code
editor tabs, the event-handler editor, the COBOL Structure block editors
(SPECIAL-NAMES / REPOSITORY / FILE SECTION / WORKING-STORAGE), all of which
share `EditorPanel`'s ✨ Beautify, and the Indexed editor's Beautify button
(which regenerates canonical text from the parsed record and is already
error-gated; its emitted columns must agree with rules 3 and 5).

## The rules (operator's words, normalized)

1. Do not beautify code within `EXEC … END-EXEC` (interior lines pass
   through verbatim; the `EXEC` / `END-EXEC` lines themselves are placed by
   the normal rules).
2. COBOL paragraphs at column 8.
3. Level numbers: `01` (and `77`/`78`/`88` top-level) at column 8; nested
   levels 02–49 indent **3 spaces per depth step** based on the previous
   indentation.
4. A data-item declaration occupies **one line** (wrapped clause lines are
   joined), e.g. `05 company-name   IS GLOBAL PIC X(40) VALUE "IPSUM LOREM".`
5. Align `PIC`/`PICTURE` and `VALUE`/`VALUES` (case-insensitive) to start on
   the **same column across a run of consecutive declaration rows**; never
   glue PIC to the name if that breaks the run's vertical alignment; never
   move PIC to the next line.
6. Column-7 continuation stays valid; continued text must be quote-enclosed;
   emitted lines are capped at **256 chars** (literals split via column-7
   `-` with re-quoted pieces; non-literal overflow wraps at a word
   boundary).
7. Procedure code starts at column 12.
8. Indent procedure code Python-style (one step per nesting level, 4
   spaces); scope delimiters (`END-IF`, `END-PERFORM`, `END-TRY`, …) align
   with their opening verb's column; `ELSE` / `WHEN` / `CATCH` / `FINALLY`
   align with their opener. **If the code has errors, do not beautify at
   all — warn the developer instead.**
9. Add a missing period **only where necessary**: before a paragraph
   header, before `CATCH`/`FINALLY` in `TRY/CATCH/FINALLY`, and at the end
   of a data entry followed by a new entry. Never duplicate a period.
10. Before formatting, a modal confirms: (a) COBOL verbs left alone /
    UPPERCASE / lowercase / Capitalized; (b) comments left alone or aligned
    to the surrounding code column.
11. Undo restores the pre-beautify state.
12. A `SECTION` header is preceded by one blank line (added 2026-08-07,
    same session; never doubled).

## Recorded decisions (assumptions the operator can veto)

- **Error gate**: full-program texts (containing an `IDENTIFICATION`/`ID
  DIVISION`) are gated by the real lexer+parser (any `Severity::Error`
  rejects). Fragments (structure blocks, handlers) are gated structurally:
  unterminated string literal, `EXEC` without `END-EXEC`, `END-x` with no
  open scope. Rejection lists the findings in a dialog; the text is left
  untouched.
- **Alignment runs** (rule 5) break at blank lines, comments, `EXEC`
  blocks, section/FD headers. The PIC column is the run's widest
  name-part end + 2; the VALUE column is the widest PIC end + 2 (level-88
  `VALUE` participates in the same VALUE column).
- **Modal cadence**: shown on every Beautify click; both choices are
  remembered across sessions (`ui_prefs`) as the next defaults.
- **Comments**: "aligned" moves only free-form `*>` comments to the current
  code column; classic column-7 `*` / `/` indicator comments stay pinned at
  column 7 in both modes.
- **Undo**: the editor's normal undo (the text widget records the
  programmatic replacement as one undo step).
- Verb casing touches reserved words only — never identifiers, literals,
  comments, or PIC masks.
