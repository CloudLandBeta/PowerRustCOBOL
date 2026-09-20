# Spec — Printing a report into a Viewer

- **Status:** draft → **ready for `/clarify` or `/plan`** (three design questions
  answered by the operator 2026-09-20; the rest are stated assumptions in §7)
- **Folder:** specs/062-report-to-viewer/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-20

## 1. Overview

A COBOL report has two destinations today: a printer (`ASSIGN TO PRINTER`) and a
file (`ASSIGN TO "path"`). Both leave the developer with the same problem — the
report exists somewhere else, and showing it to the operator means finding it,
opening it in something, and hoping that something renders it.

This spec adds a third destination: **a Viewer control on the running form**.

```cobol
       SELECT REPORT-FILE ASSIGN TO VIEWER "VWR-1"
           ORGANIZATION IS MARKDOWN.
```

Everything from `FD` to `CLOSE` is ordinary COBOL — the same record description,
the same `OPEN OUTPUT`, the same `WRITE`. What changes is where the lines land
and how they are read:

| `ORGANIZATION IS` | The report is read as | Page control |
|---|---|---|
| `SEQUENTIAL` | plain text | **yes** — traditional COBOL print page control |
| `MARKDOWN` | Markdown, rendered | no |
| `HTML` | HTML, rendered (the Viewer's subset — no CSS) | no |

The developer writes the report the way they always have; the reader gets a
formatted document in a control that already knows how to zoom, search, split,
print and save it.

**Classification: FEATURE** (GOLDEN RULE #5). This is a capability beyond the
COBOL-85 standard and beyond the IDE's existing scope — a new `ASSIGN` device,
two new `ORGANIZATION` values, and a bridge from the file-I/O layer to a live
control. It belongs on the `features` branch, in commits of its own, and on
forum **f=96** if it is ever announced. See §6 for the branch problem that must
be settled before `/implement`.

## 2. Goals / Non-goals

**Goals**

- `ASSIGN TO VIEWER "<control-id>"` names the Viewer that receives the report.
- `ORGANIZATION IS MARKDOWN` and `ORGANIZATION IS HTML` join the existing four.
- Nothing else about the file changes: `FD`, record descriptions, `OPEN`,
  `WRITE`, `CLOSE` are what they already are.
- The report is a **real file** on disk — `<form-name>-<unique>.<ext>` in the
  OS temp directory — so everything the Viewer can already do to a document
  (Save As, Print, Share, search, split) works on a printed report with no new
  machinery. When the temp directory cannot be written, the report is held in
  memory and still displayed.
- `SEQUENTIAL` keeps traditional print page control, and a page break is a page
  the reader can see.

**Non-goals**

- **Reading** from a Viewer. `OPEN INPUT` / `I-O` on a `VIEWER` file is refused.
- Changing `ASSIGN TO PRINTER`, or any existing organization.
- CSS, scripts, or any HTML beyond the subset the Viewer already renders.
- Page control for `MARKDOWN` / `HTML` (§4 R12 says what happens instead).
- Targeting view 2 of a split, or several Viewers from one file. View 1 always
  (operator, 2026-09-20). A `VIEW n` phrase is a later spec if it is wanted.
- Rendering the report anywhere but a Viewer — no print-preview window, no
  export dialog. The Viewer's own Print already reaches the platform (1.70.126).

## 3. User stories

- As a COBOL developer, I want my existing report-writing code to display its
  output on the form, so that I do not have to leave the program to show a
  result.
- As a COBOL developer, I want to write Markdown from COBOL, so that a report
  can have headings and tables without me generating HTML by hand.
- As a COBOL developer, I want `AFTER ADVANCING PAGE` to still mean a page, so
  that a report I already have prints on screen the way it prints on paper.
- As an operator, I want the report on screen to be a document — searchable,
  zoomable, savable — and not a picture of one.

## 4. Requirements (EARS)

### The declaration

- **R1 (ubiquitous):** The system shall accept `ASSIGN TO VIEWER <literal>` in a
  `SELECT`, where the literal is the `id` of a Viewer control on the form.
- **R2 (ubiquitous):** The system shall accept `ORGANIZATION IS MARKDOWN` and
  `ORGANIZATION IS HTML`, with or without the optional words `ORGANIZATION IS`,
  exactly as the four existing organizations are accepted.
- **R3 (constraint):** The system shall not change the meaning of
  `ASSIGN TO PRINTER`, `ASSIGN TO <literal-path>`, or of `SEQUENTIAL`,
  `LINE SEQUENTIAL`, `RELATIVE` and `INDEXED` on a file that is not a `VIEWER`
  file.
- **R4 (event):** When a `SELECT` names `ORGANIZATION IS MARKDOWN` or `HTML`
  without `ASSIGN TO VIEWER`, the system shall report a diagnostic naming the
  file — these organizations describe how a Viewer reads a document, and a disk
  file written that way would be an ordinary text file with a misleading
  declaration.

### Writing the report

- **R5 (ubiquitous):** The system shall treat the `FD`, its record descriptions,
  `OPEN`, `WRITE`, `CLOSE` and the file's `FILE STATUS` exactly as it treats a
  print file, so an existing report program needs no change but its `SELECT`.
- **R6 (ubiquitous):** Each `WRITE` shall contribute one line of text, taken
  from the record area, with trailing spaces removed and every other character
  kept — a report's columns are made of leading and intervening spaces, and
  padding to the record length would put nothing on the page but bytes.
- **R7 (event):** When a program executes `OPEN OUTPUT` on a `VIEWER` file, the
  system shall begin a new, empty document.
- **R8 (event):** When a program executes `OPEN EXTEND` on a `VIEWER` file whose
  document already exists, the system shall append to it.
- **R9 (event):** When a program executes `OPEN INPUT` or `OPEN I-O` on a
  `VIEWER` file, the system shall refuse it with a file status the program can
  test, and shall not display anything.

### Where the report lives

- **R10 (ubiquitous):** The system shall back the report with a file named
  `<form-name>-<unique>.<ext>` in the operating system's temporary directory,
  where `<ext>` is `txt` for `SEQUENTIAL`, `md` for `MARKDOWN` and `html` for
  `HTML` (operator, 2026-09-20).
- **R11 (state):** While the temporary directory cannot be written — unset,
  missing, read-only, full — the system shall hold the report in memory and
  display it from there, so a report is never lost to a filesystem problem.
- **R12 (constraint):** The system shall not delete the backing file when the
  form closes. It is the operating system's to reap, and deleting it would break
  a Save As the reader had not got to yet.

### Showing it

- **R13 (event):** When a program executes `CLOSE` on a `VIEWER` file, the
  system shall display the finished document in **view 1** of the named Viewer
  (operator, 2026-09-20).
- **R14 (state):** While a `VIEWER` file is open, the system shall leave the
  Viewer showing whatever it was showing — a report appears when it is finished,
  not as it is written.
- **R15 (constraint):** The system shall not change the Viewer's `Layout`,
  `Zoom`, `SplitMode`, `FontSize` or any other property the developer set. The
  organization decides how the bytes are read; the developer decides how the
  document is presented.
- **R16 (event):** When a second report is closed into the same Viewer, the
  system shall replace the document it is showing.
- **R17 (ubiquitous):** The system shall make a printed report behave like any
  other document in the Viewer — Save As, Print, Share, search, zoom and the
  card grid all work on it, and Save As writes the bytes the report actually
  contains.

### Page control

- **R18 (state):** While the organization is `SEQUENTIAL`, the system shall
  honour `WRITE … AFTER/BEFORE ADVANCING n LINES`, `ADVANCING PAGE`, the `FD`'s
  `LINAGE` clause and `AT END-OF-PAGE` exactly as it honours them for a print
  file today.
- **R19 (event):** When a `SEQUENTIAL` report advances to a new page, the system
  shall start a new page in the Viewer's `Page` layout, so the page breaks the
  program wrote are the pages the reader turns.
- **R20 (constraint):** The system shall not silently ignore page control on a
  `MARKDOWN` or `HTML` file. `ADVANCING PAGE`, `LINAGE` and `END-OF-PAGE` on
  such a file shall be reported as a diagnostic — the developer asked for
  something the organization cannot do, and a report that quietly loses its page
  breaks is worse than one that will not compile.

### When it cannot work

- **R21 (event):** When the named control does not exist, is not a Viewer, or
  the program is not running on a form at all, the system shall fail the `OPEN`
  with a file status the program can test and a message naming the control id.
- **R22 (event):** When a program closes a `VIEWER` file to which nothing was
  written, the system shall display an empty document rather than leaving the
  previous one in place — the report ran and produced nothing, and that is a
  result.

## 5. Acceptance criteria

- [ ] **AC1** — `SELECT R ASSIGN TO VIEWER "VWR-1" ORGANIZATION IS MARKDOWN.`
      parses, passes `rcrun check`, and the AST records the device, the control
      id and the organization. (R1, R2)
- [ ] **AC2** — The same program with `ORGANIZATION IS SEQUENTIAL` and with
      `HTML` parses identically; `ORGANIZATION MARKDOWN` (words omitted) parses
      too. (R2)
- [ ] **AC3** — A report program that writes 40 lines and closes leaves a file
      in the temp directory named `<form>-<unique>.md`, containing exactly those
      40 lines, trailing spaces removed. (R6, R10)
- [ ] **AC4** — With the temp directory made unwritable, the same program still
      displays the report. (R11)
- [ ] **AC5** — On `CLOSE`, view 1 of `VWR-1` shows the rendered document: a
      Markdown heading is painted as a heading, an HTML table as a table, and a
      `SEQUENTIAL` report as monospaced plain text. (R13)
- [ ] **AC6** — Nothing is shown before `CLOSE`: a test that writes 10 records
      and asserts the Viewer's source is unchanged passes. (R14)
- [ ] **AC7** — The Viewer's `Layout`, `Zoom`, `SplitMode` and `FontSize` are
      the same after the report as before it. (R15)
- [ ] **AC8** — A second report closed into the same Viewer replaces the first;
      the first's backing file still exists on disk. (R16, R12)
- [ ] **AC9** — `SaveAs()` on a printed report writes a file byte-identical to
      the backing file. (R17)
- [ ] **AC10** — A `SEQUENTIAL` report with `LINAGE IS 20 LINES` and
      `WRITE … AFTER ADVANCING PAGE` produces the same bytes as the same program
      writing to a disk file, and `AT END-OF-PAGE` fires on the same records.
      (R18)
- [ ] **AC11** — That report, shown in `Page` layout, breaks pages where the
      program broke them — asserted on the painted page count, not on the bytes.
      (R19)
- [ ] **AC12** — `ADVANCING PAGE` on a `MARKDOWN` file is reported by
      `rcrun check`, naming the file and the line. (R20)
- [ ] **AC13** — `ASSIGN TO VIEWER "NOPE"` fails the `OPEN` with a testable file
      status and a message naming `NOPE`; the program keeps running and can act
      on it. (R21)
- [ ] **AC14** — `OPEN INPUT` on a `VIEWER` file is refused the same way. (R9)
- [ ] **AC15** — A console program (`rcrun run`) with a `VIEWER` file fails the
      `OPEN` rather than crashing. (R21)
- [ ] **AC16** — The behaviour is identical under `rcrun run-form`, in an
      embedded child form, and in a **compiled binary** — the three hosts that
      construct the form interpreter. (see §6)

## 6. Constraints & steering check

**i18n.** No new IDE surface is expected — the developer writes COBOL and the
report appears in a control that already exists. Any string that *does* reach
the IDE (an error in the output panel, a designer hint) must be a `Tr` field in
all six languages. Runtime diagnostics carried in a `FILE STATUS` are not UI
strings and stay English, like every other runtime message.

**Generated code.** Unaffected. This is hand-written COBOL in Common Code or an
event handler; `cobolt-codegen` generates no `SELECT` of its own for it.

**Docs.** The Developer's Guide needs a section — the audience is a PowerCOBOL /
isCOBOL developer who knows `ASSIGN TO PRINTER`, and the whole feature is an
extension of an instinct they already have. English canonical only; the Guide's
translations are already absent, so GOLDEN RULE #8 costs nothing here. The
System KB's `cobolt-compiler` tables describe controls, properties, methods and
events — a Viewer gains none of those, so `chunked.data` is expected to be
unchanged; if any table text does change, it must be regenerated in the same
change.

**Three hosts, one behaviour.** A running form's behaviour must reach `rcrun
run-form`, embedded child forms **and** the compiled binary (`interpreter-binary-parity`).
AC16 exists to stop this being remembered only for the first two.

**Fix vs feature.** Feature — see §1. Two consequences:

1. It goes on `features`, in commits separate from any fix.
2. **`origin/features` is stale at 1.70.113 and is checked out in another
   worktree** (`.claude/worktrees/viewer`), so it cannot simply be checked out
   here, and it carries feature commits (1.70.111, 1.70.113) whose presence on
   `main` has not been verified. How this lands — sync `features` from `main`
   first, or branch off `main` and push to `features` — must be settled before
   `/implement` writes a line.

**What already exists**, so `/plan` does not cost more than it must:

- `ASSIGN TO VIEWER` already *parses*: the ASSIGN target is captured as a plain
  word (`parse_file_control_entry`), so the device needs recognising, not lexing.
- `MARKDOWN` / `HTML` arrive as identifiers; `parse_organization` needs two more
  arms, not two new tokens.
- **`LINAGE`, `ADVANCING` and `AT END-OF-PAGE` are already implemented** —
  `Interpreter::advance_linage`, `linage_counters`, `FileSpec::linage`. R18 is
  reuse.
- The Viewer classifies a document **by file extension** with a content sniff
  behind it (`viewer::ViewerFormat`), so R10's `.md` / `.html` / `.txt` naming
  is all it takes to get the right renderer.
- A Viewer session is keyed by **path** and a source that changes to a new path
  loads it, so R13 is a property write over the state channel the interpreter
  already uses — not a new transport.

## 7. Open questions

Stated assumptions, each safe to proceed on and cheap to reverse:

1. **`.md` for MARKDOWN.** The operator wrote `<txt/html>`; the Viewer reads
   `.md`/`.markdown` as Markdown, and writing Markdown bytes into a `.html` file
   would make every other tool wrong about them. Assumed `.md`.
2. **What `<unique>` is.** No crate in the workspace depends on `uuid` directly.
   A v4 UUID means a new dependency; pid + timestamp + counter means none.
   `/plan` decides; the requirement only asks that two reports never collide.
3. **The exact file statuses** for R9, R21 and R22. `37` (open mode not
   supported) and `35`/`39` are the candidates; the plan should pick from the
   set `status.rs` already uses rather than invent one.
4. **Whether R20's diagnostic is semantic or runtime.** A semantic diagnostic at
   `rcrun check` is more useful, but page control can be reached through a
   `LINAGE` on an `FD` whose `SELECT` is far away; the analyser has to join them
   up. If that proves awkward, a runtime status is the fallback.
5. **`OPEN EXTEND` (R8)** — assumed to append to the same backing file. Worth a
   moment in `/clarify`: the alternative is that EXTEND is refused like INPUT.
6. **Whether a report should also be reachable as a plain file** by the
   developer's own code (`ASSIGN TO "report.md" ORGANIZATION IS MARKDOWN`). R4
   currently forbids it. It is a small extension if it is wanted, and a
   confusing half-feature if it is not.
