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

1. Do not beautify code within a ``` ``` ``` **block literal** — the fences
   and every line between them pass through verbatim, byte for byte, and the
   rule-6 line cap is not applied to them. The text between the fences is the
   literal's value (`Lexer::capture_block_literal` takes it with no escaping),
   so any reformatting inside changes what the program moves. An unclosed
   fence is an error under rule 8. *(Operator, 2026-09-07.)*
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

## Addendum — the PowerRustCOBOL style (operator, 2026-10-12)

The modal of rule 10 gains a third choice, **PowerRustCOBOL style**, remembered
like the other two and **on by default**. Off, every rule above holds unchanged.
On, it replaces rules 3 and 5 and adds the following (the numbers are the
operator's own list for COBOL the agent writes; `layout` golden rule of the
plugin):

1. **Data levels** indent **4** spaces per depth step (rule 3 said 3); an 88
   sits one step under its item.
2. **The gutter.** The first clause after a data name — `PIC`, `OCCURS`, the
   `VALUE` of an 88, `REDEFINES`, … — starts in **column 50**, followed by 4
   spaces and its operand; later clauses follow after 4 spaces. A name too long
   to reach it sends the clauses to column 50 of the next line, with a blank
   line after (unless a scope end follows). This replaces rule 5's per-run
   alignment, and its "never move PIC to the next line".
3. **Blank lines.** Before every `DIVISION` except `ENVIRONMENT DIVISION`; before
   a comment block (operator, 2026-10-12 — none more when the line above is
   already blank, none at the top) and after it, before the code below it; before and after a block of two or more
   consecutive statements of the same verb at the same level — never at the start
   or end of a scope, and never two in a row.
4. **Comments** (`*>`) sit in the column of the code below them; the comment
   choice of rule 10b is moot with the style on. Classic column-7 comments stay.
5. **Verbs.** 4 spaces between a verb and its predicate (`SET` takes 5, so it
   lines up with `MOVE`); `EXIT PERFORM`, `STOP RUN`, `GO TO` are one verb. `TO`
   of `MOVE`/`SET` in column 50, or in column 50 of the next line when the operand
   before it is too long, with a blank line after.
6. **Structure.** A statement with conditional phrases (`READ … AT END … END-READ`,
   `STRING … END-STRING`, …) is a scope: its phrases sit one level in, their
   bodies two. A line that continues a statement (`INTO …`, `WITH POINTER …`)
   sits one level in; an `AND`/`OR` condition line two levels in from its
   header. The clauses of a `SELECT` sit one level under it. A statement split
   over lines (`MOVE x` / `TO y`, a list of targets, `INVOKE … RETURNING`) is
   treated as one.
7. **Statements** — the only rules that change tokens, kept only when proved
   harmless (below): a sentence period is dropped when no scope or phrase depends
   on it and nothing (paragraph, section, end of code) follows; `INVOKE obj::m(…)`
   becomes `obj::m(…)` — except on `ME`/`SUPER`, whose window calls run through
   a path the bare statement does not take; `MOVE` becomes `SET`, except
   `MOVE CORRESPONDING` and `MOVE ALL`.
8. **Long lines** break before a string literal that fits on a line of its own
   instead of through it (rule 6 still splits a literal that cannot).

### Recorded decisions

- **The proof.** After formatting, the text before and after are read by the real
  parser as programs (a fragment gets an identification header, and a procedure
  header when it has none) and compared with positions and sentence-end markers
  removed, and with `INVOKE obj::m(a)` and `obj::m(a)` read as one call. Equal ⇒
  the rule-7 changes stay. Unequal, or the original does not parse ⇒ the text is
  formatted again with layout alone. Layout never changes tokens.
- **Names keep their spelling.** Verb casing (rule 10a) no longer touches a word
  right before or right after `::` — `NUD-AGE::Value`, `super::GetProperty`. It
  used to turn `Value` into `VALUE`.
- **A lone call ending in a period** (`Txt-Log::AppendText(WS-NL).`) is not a
  paragraph header; a paragraph name is letters, digits, hyphens and underscores.
- **Surfaces.** Every surface that offers Beautify, not only the event editor,
  gets the style: they share the engine. The Indexed editor's Beautify, which
  regenerates canonical text, is unchanged.
- **Not done by Beautify:** *comments before complex code* — that is judgment,
  not layout.
