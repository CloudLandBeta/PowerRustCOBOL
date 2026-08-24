# Spec — Code locations: precise diagnostics and project-wide code search

- **Status:** draft → approved
- **Folder:** specs/053-code-locations-and-search/
- **Author:** Anthropic Claude Codex Agent (with the operator)   **Date:** 2026-08-23

## 1. Overview

A developer writes COBOL in nine different places in this IDE — a control's
event handler, a form's lifecycle handler, a user procedure, the five structure
sections (`SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
`WORKING-STORAGE`), and hand-written Common Code files. Codegen weaves the first
eight into one generated `.cbl` per form, and every tool downstream — lexer,
parser, semantic analyser — sees only that woven artifact.

So when something is wrong, the Output panel says `842:17: ✖ error: …`
(`DiagMsg { severity, message, line, col }` — [runner.rs:85]). Line 842 of a
file the developer never wrote, is not allowed to edit, and which is deleted and
regenerated on the next Build. There is no form name, no section, no handler, and
nothing to click. The developer's only recourse is to open the read-only
generated file, read line 842, recognise their own code, and go hunting for it in
the RAD.

The same missing idea makes the second half impossible. Search today is
`SearchState` inside one open editor tab ([editor.rs:1095]) — it cannot see a
string that lives in another form's `onClick`, in a `FILE-CONTROL` block, or in a
procedure that is not currently on screen. There is no way to answer "where is
`CUST-BALANCE` used?".

Both halves need the same thing the codebase does not have: a **code site** — a
stable address for a place where a developer can write COBOL, resolvable back to
the editor that owns it. Given that address, a diagnostic can name its site and
carry a link, and a search can enumerate every site and jump into one.

## 2. Goals / Non-goals

**Goals**

- One address (`CodeSite`) for every place a developer can write COBOL.
- A map from generated `.cbl` lines back to the code site and the line **within
  that site** that produced them.
- Compile-time diagnostics that name `Form ▸ Site`, give the line and column
  *inside the developer's own text*, quote the offending line, and — for a site
  that has an editor — open it with the caret on that line.
- A project-wide code search over every code site, results grouped and
  navigable, double-click to land in the RAD on the matching line — in its own
  persistent, user-resizable window that stays open while you work.

**Non-goals** (explicitly out of scope)

- **Runtime error locations** (operator ruling, 2026-08-23). A COBOL abort while
  a form is running keeps today's message. The source map is built so a later
  spec can carry it into execution, but nothing here depends on that.
- **Replace / bulk edit** (operator ruling). Find only. No path in this feature
  writes to developer code.
- **Searching generated `.cbl`** (operator ruling). It is a build artifact; its
  hits would be unopenable duplicates of hits already listed at their real site.
- A Problems/Errors panel. The Output panel stays the surface for diagnostics.
- Searching non-code — control captions, property values, documentation,
  project settings.
- Regex and multi-line patterns (see Open questions).

## 3. User stories

- As a COBOL developer, when Check reports a syntax error, I want to read *which
  form, which handler, and which line of my own code* is wrong, so that I can fix
  it without decoding a generated file.
- As a COBOL developer, I want to click the error and land on the offending line
  in the editor that owns it, so that finding it costs one click, not a search.
- As a COBOL developer, I want to read the error message alone and still know
  where to look, so that I am not dependent on the link (over a screenshot, a
  forum post, or a colleague's shoulder).
- As a COBOL developer maintaining an application, I want to list every place a
  data item or paragraph name appears — in any handler, procedure or structure
  section of any form — so that I can judge the blast radius of a change.
- As a COBOL developer, I want to double-click an occurrence and be taken to it
  in the RAD with the cursor on that line, so that browsing results is how I
  navigate the project.

## 4. Requirements (EARS)

### The code-site address

- **R1 (ubiquitous):** The system shall define a **code site** address that
  uniquely identifies each place a developer can write COBOL in a project:
  a control event handler (form + control id + event), a form lifecycle handler
  (form + event), a user procedure (form + procedure name), each of the five
  structure sections (form + section), and a Common Code file (project-relative
  path).
- **R2 (ubiquitous):** Each code site shall carry a **display path** for the UI,
  reading form-first and left-to-right — e.g. `Main Menu ▸ Button-1 ▸ onClick`,
  `Main Menu ▸ WORKING-STORAGE`, `Main Menu ▸ VALIDATE-CUSTOMER`.
- **R3 (ubiquitous):** A code site shall be resolvable to the editing surface
  that owns it — the event modal (`DesignerPanel::open_event_modal`), the
  structure window (`CsTarget`), or the main code editor for a Common Code file.
- **R4 (constraint):** The system shall not treat a generated `.cbl` as a code
  site. Generated code is an artifact, not a place a developer writes.
- **R5 (state):** While a form is open in the RAD with unsaved edits, a code
  site shall address the **live in-memory** text, not the last-saved `.cfrm`.

### Mapping generated code back to its origin

- **R6 (ubiquitous):** For each form it generates, codegen shall produce a
  **source map**: for every line of the generated `.cbl`, either the code site
  and the 1-based line **within that site** that produced it, or a marker that
  the line was authored by codegen itself.
- **R7 (ubiquitous):** The map shall cover every site listed in R1 that codegen
  weaves in, in the order it weaves them, including sites that appear more than
  once and sites whose text is empty.
- **R8 (constraint):** Producing the map shall not change the generated COBOL by
  a single byte. The map is metadata emitted alongside the source, never markers,
  comments or pragmas inside it.
- **R9 (ubiquitous):** The map shall survive the regenerate-on-Build/Run/Debug/
  Check contract — it is produced by the same call that produces the `.cbl`, so
  the two cannot drift.

### Diagnostics that say where

- **R10 (event):** When a Check, Build, Run or Debug produces a parser or
  semantic diagnostic for a generated form program, the Output panel shall show
  the owning code site's display path, and the line and column **within that
  site**, instead of the generated file's line and column.
- **R11 (event):** When such a diagnostic is shown, the Output shall also quote
  the offending source line and mark the column, so the message locates the fault
  on its own, with no link followed and no file opened.
- **R12 (event):** When a diagnostic maps to a line codegen authored (R6), the
  Output shall say so explicitly and give the generated file and line — it shall
  not attribute generated plumbing to the developer, nor guess a nearby site.
- **R13 (event):** When the developer activates a diagnostic that has a code
  site, the system shall open that site's owning editor, focused, with the caret
  on the mapped line and the mapped column selected.
- **R14 (event):** When the developer activates a diagnostic for a Common Code
  file, the system shall open that file in the main editor at the mapped line.
- **R15 (constraint):** Activating a diagnostic shall not open, focus or scroll
  the generated `.cbl`.
- **R16 (ubiquitous):** A diagnostic that can be activated shall be visibly
  distinguishable from one that cannot, and shall respond to hover, so the
  developer can see what is clickable before clicking.

### Project-wide code search

- **R17 (ubiquitous):** The IDE shall provide a project-wide code search that
  enumerates occurrences of a plain-text query across **every** code site in R1,
  in every form of the open project plus every Common Code file.
- **R18 (ubiquitous):** Search shall offer case-sensitive and whole-word
  options, both off by default.
- **R19 (ubiquitous):** Each result shall show its code site's display path, the
  1-based line number within that site, and the text of the matching line with
  the matched span highlighted.
- **R20 (ubiquitous):** Results shall be grouped by form, then by site, in a
  stable order, and the total number of occurrences and of distinct sites shall
  be shown.
- **R21 (event):** When the developer double-clicks a result, the system shall
  open the RAD for that result's form (opening the designer if it is not already
  open), open the owning editor for that site, and place the caret on the
  matching line with the match selected.
- **R22 (state):** While a form has unsaved edits in the RAD, search shall read
  its live in-memory text, so a result can never point at a line that is no
  longer there.
- **R23 (constraint):** Search shall not modify any developer code, and shall
  offer no path that does.
- **R24 (constraint):** Search shall exclude generated `.cbl` files and the
  deleted-code recycle bin (`Form::deleted_code`).
- **R25 (event):** When a query matches nothing, the panel shall say so
  explicitly rather than showing an empty list.

### Responsiveness

- **R26 (constraint):** Search shall not block the UI. Where a query cannot be
  answered within a single frame, it shall run off the paint path and report
  progress, and the IDE shall stay interactive throughout.
- **R27 (ubiquitous):** The search test shall report **measured** timings and
  counts — forms scanned, sites scanned, occurrences found, elapsed ms — for a
  project large enough to be meaningful, per the steering rule on quantified
  test output.

### Non-regression

- **R28 (constraint):** The existing in-tab editor search (`SearchState`) shall
  keep working unchanged.
- **R29 (constraint):** The existing double-click-an-event → jump behaviour
  (pending task #70, `jump_to_event_code`) shall keep working; where it is
  re-pointed at the new address it shall lose no behaviour.
- **R30 (constraint):** Generated `.cbl` output shall be byte-identical to
  today's for every form in the test corpus (the guard for R8).
- **R31 (ubiquitous):** Every new user-facing string shall be a `Tr` field
  translated in all six languages (EN/ES/PT/JA/ZH/FR).

### The search window

- **R32 (ubiquitous):** The search shall live in its **own window**, not docked
  into an existing IDE pane, so its layout is free of the panes around it
  (operator ruling, 2026-08-23).
- **R33 (ubiquitous):** The window shall open at a sensible default size and be
  resizable on both axes by **dragging a grip with the mouse**. Resizing is the
  user's act: nothing else changes the window's size.
- **R34 (constraint):** The window shall **hold the size it was given** — the
  default until the user drags, then whatever the user dragged it to. It shall
  not grow, shrink or drift on its own across frames, queries, result counts, or
  a change of font size.
- **R35 (state):** While the search window is open it shall **stay** open —
  through a jump to a result, a click elsewhere in the IDE, a rebuild, or a
  form being opened or closed. It shall close only when the user clicks the
  window's close control (`✕`) or its **Cancel** button.
- **R36 (constraint):** The window shall not block input to the rest of the IDE.
  Navigating to a result (R21) puts the caret in an editor *behind* this window,
  and a window that swallowed input would make its own main feature unusable.

## 5. Acceptance criteria

- [x] **AC1** — A code site exists for all nine kinds in R1, and each renders a
      display path matching R2. Round-trip test: site → display path → resolve →
      the same site.
- [x] **AC2** — For a form with code in *every* site kind, the source map
      accounts for 100 % of the generated lines: each is attributed to a site +
      line, or explicitly to codegen. No line is unattributed.
- [x] **AC3** — A deliberate syntax error placed in each site kind in turn is
      reported at the correct site with the correct line *within that site*
      (±0 lines), for all nine kinds.
- [x] **AC4** — The generated `.cbl` for the corpus is byte-identical before and
      after this feature (R8/R30).
- [x] **AC5** — The Output row for a form diagnostic contains: form name, site
      display path, line, column, and the offending source line. Asserted on the
      rendered text, not on internal state.
- [x] **AC6** — Activating that row opens the site's owning editor with the caret
      on the mapped line; the generated `.cbl` is not opened (R15).
- [x] **AC7** — A diagnostic on a codegen-authored line is labelled as generated
      code and names the generated file and line — and is *not* attributed to any
      developer site.
- [x] **AC8** — Search for a string that occurs in a control handler, a form
      handler, a user procedure, `WORKING-STORAGE`, `FILE-CONTROL`, `REPOSITORY`,
      `SPECIAL-NAMES`, `FILE SECTION` and a Common Code file finds **all nine**,
      each with the right site path and line number.
- [x] **AC9** — Case-sensitive and whole-word options change the result set as
      specified, verified with a query that distinguishes them
      (e.g. `bal` vs `BAL` vs `CUST-BAL`).
- [x] **AC10** — Search finds a string typed into an open form's handler but
      **not yet saved** (R22).
- [x] **AC11** — Search returns no hits from a generated `.cbl` or from recycled
      deleted code, proven with a string that exists only there.
- [x] **AC12** — Double-clicking a result for a form that is *not* open in the
      RAD opens its designer and lands on the matching line.
- [x] **AC13** — A zero-match query shows the explicit no-matches state (R25).
- [x] **AC14** — The search test prints measured counts and timings (R27), and
      the reported numbers are ones the run produced.
- [x] **AC15** — Every new string is present in all six languages; the i18n
      completeness test passes.
- [x] **AC16** — In-tab editor search and the event double-click jump behave as
      before (R28/R29).
- [x] **AC17** — Over 120 rendered frames, with a result list long enough to
      overflow, the search window's size does not drift by more than 0.5 px after
      it settles, and settles near its seeded size (R34 — the self-inflation
      guard).
- [x] **AC18** — Dragging the grip resizes the window on both axes, and the new
      size survives a re-query and a result list of a very different length
      (R33/R34).
- [x] **AC19** — The window is still open after: jumping to a result, clicking
      in the editor behind it, and re-running Check. It closes on `✕` and on
      Cancel, and on nothing else (R35).

## 6. Constraints & steering check

- **i18n (6 languages):** yes — a new search panel, its options, its empty state,
  and the new diagnostic phrasing are all user-facing. Every string is a `Tr`
  field in `i18n.rs` across EN/ES/PT/JA/ZH/FR (R31, AC15).
- **Generated-code / regenerate contract:** untouched. The source map is produced
  by the same generation call and emitted as metadata; the `.cbl` bytes do not
  change (R8), pinned by AC4. Generated code stays read-only and is never a
  navigation target (R4, R15).
- **Docs:** `docs/developers-guide-en.md` gains a section on reading a
  diagnostic (what the site path means, generated-code lines) and one on the
  search window. **English only — the translations are not touched**
  (operator ruling, 2026-08-23, settling the three-way steering contradiction in
  favour of `tech.md` / `structure.md`; `specs/steering/docs.md`'s
  "Claude-maintained" line is the one that is now wrong and should be corrected
  in this change).
  *This governs documentation, not the IDE's UI strings — those remain `Tr`
  fields in six languages, which is a separate hard constraint and a stated
  product goal.*
- **System KB:** no control, property, method or event changes, so the KB doc
  tables are untouched and no `chunked.data` rebuild is implied. `/plan` must
  re-check this if it ends up changing any documented behaviour.
- **Fix vs feature:** **feature** — a new IDE capability (announce on f=96,
  branch `feat/`). ⚠️ The diagnostics half is arguable: "errors point at a build
  artifact the developer cannot edit" reads as a defect, and Rule #5 forbids
  mixing a fix and a feature in one commit. Recommendation: ship the whole spec
  as a feature, since the mapping infrastructure is genuinely new. If the
  operator classes the diagnostics half as a fix, the plan must split it into its
  own branch and commit, landing the source map first.
- **Versioning:** fix number `z` in `crates/cobolt-ide/src/version.rs` only, plus
  a `CHANGELOG.md` entry. Only the operator raises `x` or `y`.
- **Crate placement:** the site address and source map belong with the model and
  the generator (`cobolt-forms`, `cobolt-codegen`) so the CLI can use them too;
  the panel, the Output link and the navigation belong in `cobolt-ide`
  (`panels/` + wiring in `app.rs`), per `structure.md`.

## 7. Open questions

- ~~**Q1 — Where does the search UI live?**~~ **Resolved** (operator,
  2026-08-23): its own window, so its layout owes nothing to the existing panes.
  Behaviour is now specified in R32–R36.
- **Q2 — How is search invoked?** A menu action, a toolbar button, a keyboard
  shortcut, or all three? If a shortcut, it must not collide with the in-tab
  search (R28) — note the recorded egui pitfall that `consume_key` ignores extra
  modifiers, so a plain-key binding must match modifiers exactly.
- **Q3 — Should the recycle bin be searchable behind an explicit opt-in?**
  R24 excludes `Form::deleted_code`. Code preserved from a deleted control is
  still the developer's, and "I know I wrote this somewhere" is exactly when it
  matters. An opt-in read-only toggle would cover it without cluttering normal
  results.
- **Q4 — Regex / whole-file patterns?** Out of scope here (§2). Worth confirming
  plain text + case + whole word is enough for the first cut.
- **Q5 — Single-click or double-click to activate a diagnostic?** R13 says
  "activates"; the search side is specified as double-click (the operator's
  wording). Diagnostics may want single-click, which is the usual convention for
  a link in a log.
- **Q6 — Does the map need to survive into the compiled binary?** Not needed for
  this spec (runtime locations are a non-goal), but if the answer is likely
  "yes, later", the map's shape should be serialisable now rather than
  retrofitted.
- **Q7 — What is the corpus for AC3/AC4?** A dedicated fixture form exercising
  all nine site kinds, the existing `tests/` forms, or the operator's
  PowerDemo3? AC4's byte-identical claim is only as strong as the corpus.
