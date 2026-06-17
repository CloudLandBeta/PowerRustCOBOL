# Spec — Indexed File Editor & Grid Browser

- **Status:** implemented (2026-06-15; layout revision 2026-06-16)
- **Folder:** specs/002-indexed-file-editor/
- **Author:** Emerson Lopes   **Date:** 2026-06-15

## 1. Overview

PowerRustCOBOL already treats indexed (ISAM) files as a first-class runtime
resource ([`docs/developers-guide-en.md`](../../docs/developers-guide-en.md) §14,
[`docs/indexed-file-format.md`](../../docs/indexed-file-format.md)), but the IDE
offers no visual way to **define**, **inspect**, or **browse** them. Today a
developer must hand-write `SELECT`/`FD` clauses and manage data paths manually.

This feature adds an **Indexed File Editor** — a Form Designer–like RAD surface —
plus an **Indexed File Grid Browser** for viewing and editing live records. Together
they let developers create indexed-file definitions in the project tree, import
existing on-disk files, lock structural schema after creation, and map each field to
a Form Designer control for type-aware grid editing.

## 2. Goals / Non-goals

### Goals

- Add an **Indexed Files** category to the project tree (**after Forms, before
  Common Code**) listing every indexed-file definition in the project.
- Provide an **Indexed File Editor** viewport with a **four-pane layout**: a top
  **toolbar**, a left **structure** pane (file root + indented record tree), a middle
  **label** pane (property captions), and a right **value** pane (inputs and
  read-only fields). Selection in the structure pane drives label/value content
  (file-level, group field, or leaf field), matching the Form Designer's
  selection-driven interaction model.
- Support **New Indexed File** — a guided flow to define all supported structural
  metadata (record layout, keys, storage, compression, persistence, comments) and
  create the initial on-disk data file.
- Support **Import existing…** — register an external indexed data file; its
  **File path** property points at the real location on disk; structure is loaded
  from the file's self-describing schema where available
  (`IndexedFile::inspect_path`).
- Enforce **structural immutability** after creation: field types, offsets, key
  layout, storage mode, and compression cannot change; only **non-structural**
  metadata (field comments, per-field grid-browser control choice) remains editable.
- Provide an **Indexed File Grid Browser** — open a project's indexed data file,
  show records in a scrollable grid, format columns by COBOL field type, and allow
  in-place record editing using the mapped Form Designer controls.
- Persist definitions as **`.cidx`** XML artifacts (same serialization style as
  `.cfrm`) and register paths in `cobolt.toml`.
- **Regenerate** COBOL `SELECT`/`FD` declarations from `.cidx` on Build / Run /
  Debug / Check (same contract as form codegen).

### Non-goals

- **Fujitsu / RM COBOL binary import** or byte-level conversion (future importer;
  see [`indexed-file-format.md`](../../docs/indexed-file-format.md)).
- Changing indexed **runtime engines**, WAL, or crash recovery (see
  [`specs/001-indexed-wal-crash-recovery/`](../001-indexed-wal-crash-recovery/)).
- **Structural edits** on finalized definitions (rename keys, add fields, change
  PIC, toggle `STORAGE IS DISK` → `MEMORY`, etc.) — requires recreating the file.
- **Full runtime support** for every representable key encoding / composite-key
  combination at launch; the editor may *record* such schema, but finalize shows a
  **warn-only** notice when the current runtime cannot exercise them at run time.
- **Multi-writer** or networked indexed-file administration.
- Hand-editing generated COBOL produced from `.cidx`.

## 3. User stories

- As a **COBOL developer**, I want indexed files listed in the project tree like
  forms, so I can find and open them without hunting paths in source.
- As a **COBOL developer**, I want to **create** a new indexed file visually
  (fields, keys, storage, compression), so I do not have to memorise every
  `SELECT` clause extension.
- As a **COBOL developer**, I want **structural definitions locked** after I
  finalize creation, so accidental edits cannot desynchronise the `.cidx`
  definition from the on-disk schema and cause FILE STATUS `39` on `OPEN`.
- As a **COBOL developer**, I want to add **comments** and choose a **grid control**
  per field after creation, so the browser edits dates, amounts, and flags with the
  right control without changing the COBOL layout.
- As a **COBOL developer**, I want to **import** an existing `customers.idx` from
  disk, see its schema in the editor, and have the project reference the real path,
  so I can adopt legacy data without copying bytes into the repo.
- As a **COBOL developer**, I want to **open the grid browser** on an indexed
  file and see all records in a table with type-correct columns, so I can sanity-check
  data during development.
- As a **COBOL developer**, I want to **add, edit, and delete records** in the grid
  using the same controls as the Form Designer (TextBox, CheckBox, DatePicker, …), so
  data maintenance feels consistent with the rest of the IDE.
- As a **maintainer**, I want `.cidx` → COBOL regeneration on Build/Run, so indexed
  definitions stay the single source of truth like `.cfrm` forms.

## 4. Requirements (EARS)

### Project model & tree

- **R1 (ubiquitous):** The IDE shall show a top-level **Indexed Files** category in
  the project tree (**after Forms, before Common Code**), listing every indexed-file
  definition registered in `cobolt.toml`.
- **R2 (ubiquitous):** `cobolt.toml` shall gain an `indexed` (or equivalent) file
  list under `[files]`, holding project-relative paths to `.cidx` definition
  artifacts.
- **R3 (event):** When the user chooses **Indexed Files ➕**, the IDE shall open a
  **New Indexed File** dialog (name, assign path, storage options) and, on confirm,
  create the `.cidx` artifact, register it in `cobolt.toml`, and open the editor.
- **R4 (event):** When the user chooses **Import existing…** on Indexed Files, the
  IDE shall let them pick an on-disk indexed data file, infer schema via
  `IndexedFile::inspect_path` when possible, create a matching `.cidx` stub, and
  store the data-file path in the definition's **File path** property: **project-relative**
  when the file lies under the project root, **absolute** otherwise.

### Indexed File Editor (designer)

- **R5 (ubiquitous):** Selecting an indexed-file entry in the tree shall open the
  **Indexed File Editor** in its own viewport (same multi-window pattern as the Form
  Designer).
- **R5a (ubiquitous):** The editor viewport shall apply the **same IDE colour theme**
  as the main shell (per-project `ide.theme`, including live preview while Settings is
  open). Child viewports shall use the opaque backdrop compositing used by the Form
  Designer so panel gaps never show the OS clear colour.
- **R5b (ubiquitous):** The editor shall reserve a dedicated **toolbar pane** at the
  top of the viewport (fixed height, vector-icon buttons with text tooltips) for
  Save, Save & Generate, field add/remove, raw record edit, Finalize, and Open Grid
  Browser.
- **R6 (ubiquitous):** The **structure pane** (left) shall list:
  1. **Indexed File Properties** as the root selectable item (file-level selection);
  2. the record **field tree** in order (01-level and subordinate levels indented).
  The current item shall be marked with a **`>`** prefix in the marker column;
  selecting a row updates the label and value panes. While the definition is not
  finalized, Tab / Shift+Tab (and +/- keys) shall indent/outdent the selected field
  in the flat structure list.
- **R7 (ubiquitous):** Property editing shall use **two adjacent panes** to the right
  of the structure pane:
  - **Label pane** (middle): left-aligned captions ending with `:` (e.g. `File name:`,
    `Field name:`, `PIC:`, `OCCURS:`), one row per property;
  - **Value pane** (right): the corresponding controls, combos, and read-only values.
  Content depends on the structure selection:
  - **Indexed File Properties** (file): name, assign path, access mode, record format,
    finalized flag, comments;
  - **Group field**: name, OCCURS, REDEFINES, SYNCHRONIZED, comments;
  - **Leaf field**: name, PIC, length, usage, offset, comments, grid-browser control.
  Label and value rows shall stay vertically aligned (fixed row height; taller rows
  only when a control needs extra height, e.g. custom PIC).
- **R7a (event):** When the user toggles **raw record edit** mode, the label and value
  panes shall be hidden and the centre area shall show a COBOL-like text editor for
  the full record layout; applying valid text rebuilds the structure tree.
- **R8 (ubiquitous):** A new indexed file shall allow defining, at minimum:
  - logical file name and `ASSIGN` path;
  - `ORGANIZATION IS INDEXED` access mode (`SEQUENTIAL` | `RANDOM` | `DYNAMIC`);
  - record format: **fixed length** or **variable** min/max (both exposed in the
    New Indexed File wizard);
  - field list: COBOL name, level, `PIC` category (`A`/`X`/`9`/edited), `USAGE`
    (`DISPLAY`, `COMP`, `COMP-3`, …) as supported by the language subset;
  - primary `RECORD KEY` (single field or composite parts with byte offsets);
  - zero or more `ALTERNATE RECORD KEY` entries with `WITH DUPLICATES` / `WITHOUT
    DUPLICATES`;
  - `STORAGE MODE IS MEMORY | DISK` (default **DISK** per language);
  - `WITH [DATA] COMPRESSION` (on/off);
  - `WITH PERSISTENCE` when storage is `MEMORY` (on/off);
  - free-text comments at file and field level.
- **R9 (event):** When the user **finalizes** a new indexed file (confirms the
  creation wizard / first Save after structural editing), the IDE shall:
  1. persist the `.cidx` definition;
  2. create or truncate the on-disk data file at the assign path with a schema
     matching the definition (empty file, `OPEN OUTPUT` semantics);
  3. mark the definition **finalized** (structural lock engaged);
  4. if the schema uses composite keys, non-`Bytes` key encodings, or other
     features not yet fully supported by the runtime, show a **non-blocking warning**
     (user may proceed — warn-only).
- **R10 (state):** While a definition is **finalized**, the IDE shall make
  structural properties **read-only** (field PIC/usage/length, offsets, key
  bindings, alternate keys, storage mode, compression, persistence, record format).
- **R11 (state):** While a definition is **finalized**, the IDE shall still allow
  editing **non-structural** properties: file comment, per-field comment, and
  **Grid browser control** (a `ControlType` from the Form Designer catalogue).
- **R12 (event):** When importing an existing data file, the IDE shall treat the
  imported schema as **finalized immediately** (structure read-only); non-structural
  metadata defaults (comments empty, controls auto-assigned per R15) remain editable.
- **R13 (ubiquitous):** The editor shall provide **Save** (persist `.cidx` only) and
  **Save & Generate** (persist + regenerate COBOL), mirroring the Form Designer
  toolbar pattern.

### Widget mapping & defaults

- **R14 (ubiquitous):** Per-field **Grid browser control** choices shall be limited to
  `ControlType` values already available in the Form Designer toolbox (same control
  catalogue; no parallel control set).
- **R15 (ubiquitous):** When no control is explicitly set, the IDE shall **auto-assign**
  a default control from field PIC/usage (e.g. `PIC 9` → numeric TextBox, `PIC X` →
  TextBox, `PIC A` → TextBox with alphabetic constraint, indicator `PIC 9` length 1
  → CheckBox, date-edited PIC → DatePicker where recognised).

### Grid browser

- **R16 (ubiquitous):** The Indexed File Editor shall offer **Open Grid Browser**
  (toolbar button or equivalent) for any finalized definition whose data file exists
  at the configured path; the browser shall open in a **separate OS window** (same
  pattern as the Form Designer and Run Form).
- **R17 (event):** When the grid browser opens, the IDE shall load records through
  the runtime indexed-file API (read-only or `I-O` as needed for editing) and display
  them in a **scrollable grid**: one column per leaf field, header = COBOL name.
- **R18 (ubiquitous):** Grid cells shall render values according to field PIC/usage
  (display editing picture, leading-zero suppression for numeric edited fields,
  etc.) for **display**; raw bytes are not shown unless the field is unmapped.
- **R19 (event):** When the user edits a cell (or opens a row editor), the IDE shall
  use the field's mapped **Grid browser control** (Form Designer renderer in a
  compact/grid context) for input validation and formatting.
- **R20 (event):** When the user commits a row change in the grid browser, the IDE
  shall apply the change via the runtime (`WRITE` for new rows, `REWRITE` for edits,
  `DELETE` for removed rows), honouring key uniqueness and duplicate rules, and
  surface FILE STATUS failures in the IDE output panel with a clear message.
- **R20a (ubiquitous):** The grid browser shall expose **Add row** and **Delete row**
  (or equivalent) actions in addition to in-cell editing.
- **R21 (optional):** Where the runtime supports `COMMIT`/`ROLLBACK` on the opened
  file, the grid browser shall expose **Commit** and **Rollback** actions that map
  to those verbs.

### Code generation & packaging

- **R22 (ubiquitous):** Each `.cidx` shall regenerate **exactly one** COBOL source
  file at `generated/<stem>-indexed.cbl` containing the `ENVIRONMENT DIVISION`
  `SELECT` and `DATA DIVISION` `FD`/`01` record layout matching the locked schema,
  prefixed with the standard developer banner (`cobolt-codegen::write_header`).
- **R22a (ubiquitous):** `.cidx` files shall serialize to **XML** (`.cidx`
  extension), following the same load/save patterns as `.cfrm`.
- **R23 (event):** When the user runs **Build**, **Run**, **Debug**, or **Check** on
  a project, the IDE shall regenerate all indexed-file COBOL artifacts before
  compile/run (same hook as `App::regenerate_all_forms`).
- **R24 (ubiquitous):** `rcrun package` shall include `.cidx` definitions and, when
  the data path is inside the project tree, the data files referenced by assign
  paths; external absolute paths shall be listed in packaging warnings (not
  silently omitted).

### Discovery & validation

- **R25 (event):** When opening an imported definition, if `inspect_path` returns
  `None` (legacy `PRCISAM1` / no embedded schema), the IDE shall prompt the user to
  supply or confirm schema manually before finalize, or reject import with an
  explanatory error.
- **R26 (event):** When the on-disk schema and `.cidx` structural metadata diverge
  (detected via `inspect_path` on open), the IDE shall show a blocking warning and
  refuse grid-browser writes until the user reconciles (re-import or recreate).

### Internationalisation

- **R27 (ubiquitous):** All new IDE labels, dialogs, tooltips, and error strings for
  this feature shall use `Tr` entries in **all six** languages (EN/ES/PT/JA/ZH/FR).

## 5. Acceptance criteria

- [x] **AC1 — Tree category:** A project with at least one `.cidx` shows an
  **Indexed Files** node; entries open the editor on click.
- [x] **AC2 — New file:** New Indexed File wizard produces a `.cidx`, updates
  `cobolt.toml`, creates an empty data file at the chosen assign path, and opens the
  editor with all structural fields editable **before** finalize.
- [x] **AC2a — Variable records:** New Indexed File wizard offers **Fixed** and
  **Variable** record format; choosing Variable requires min/max lengths and persists
  them in `.cidx` and generated `FD`.
- [x] **AC2b — Tree order:** Project tree shows categories in order: Forms → **Indexed
  Files** → Common Code → Generated → Assets → Documentation.
- [x] **AC3 — Finalize lock:** After finalize, attempting to change a field's PIC,
  length, or key binding is disabled (read-only UI); changing a field comment and
  grid control still works and persists across IDE restart.
- [x] **AC4 — Import:** Importing an existing `PRCIDX1`/`PRCIDXD1` file registers it
  in the tree; Properties → **File path** is project-relative when the file is under
  the project root (absolute when external); schema fields match
  `IndexedFile::inspect_path` output.
- [x] **AC5 — Properties parity:** Selecting **Indexed File Properties** vs a group vs
  a leaf field switches label/value rows analogously to Form Properties vs control
  properties (verified by UI walkthrough checklist in tasks).
- [x] **AC5a — Four-pane layout:** Editor viewport shows toolbar on top and three
  resizable columns below (structure | labels | values). Selecting a tree row updates
  both property panes; the current row shows `>` in the structure pane.
- [x] **AC5b — Theme parity:** Editor and Grid Browser viewports match the project IDE
  theme (panel fills, control colours, toolbar icon colours); no white bands between
  panes in opaque child windows.
- [x] **AC6 — Grid open:** Open Grid Browser on a populated test file opens a
  **separate window**, shows ≥1 row and column headers matching field names, and
  scrolling works for 200+ records.
- [x] **AC7 — Typed display:** Sample file with `PIC 9(5)`, `PIC X(20)`, and
  `PIC 9` indicator columns renders formatted values (not hex) in the grid.
- [x] **AC8 — Widget edit:** Changing a field's control to CheckBox and editing a row
  uses a checkbox control; invalid input is rejected before `REWRITE`.
- [x] **AC9 — Key constraint:** Attempting a duplicate primary key in the grid shows
  a FILE STATUS–backed error without corrupting the file.
- [x] **AC9a — Add/delete:** Adding a row via **Add row** persists with `WRITE`;
  deleting a row via **Delete row** removes it with `DELETE`; both survive reopening
  the grid browser.
- [x] **AC9b — Unsupported schema warning:** Finalizing a definition with a
  composite primary key shows a warn-only dialog; finalize still succeeds and the
  `.cidx` is saved.
- [x] **AC10 — Codegen:** Save & Generate emits a bannered `.cbl` whose `SELECT`/`FD`
  matches the `.cidx`; `rcrun check` on a minimal program `COPY`ing or including it
  passes semantic analysis.
- [x] **AC11 — Regenerate hook:** Build/Run on a project with indexed definitions
  refreshes generated indexed COBOL without manual steps (mtime newer than `.cidx`).
- [x] **AC12 — i18n:** No hard-coded UI strings in the new panels; spot-check ES and
  JA menu entries for Indexed Files actions.
- [x] **AC13 — Schema mismatch:** Manually corrupting a `.cidx` offset vs on-disk
  schema triggers the blocking warning (R26).

## 6. Constraints & steering check

| Steering item | Impact |
|---------------|--------|
| **i18n (6 languages)** | **Yes** — all new IDE strings via `Tr` in `i18n.rs` (R27). |
| **Generated-code contract** | **Yes** — indexed COBOL is regenerated on Build/Run/Debug/Check with `write_header` banner; never hand-edited (R22–R23). |
| **English docs** | **Yes** — extend `developers-guide-en.md` §5 (tree categories), new § for Indexed File Editor & Grid Browser, cross-link §14; add registry row in `specs/steering/docs.md` during `/docsync`. Screenshots via `/doc-shots`. |
| **Translations** | **Do not edit** `developers-guide-{es,pt,jp,cn}.md`; emit `/doc-localize` work order. |
| **Branding** | User-facing text says **PowerRustCOBOL** / **Indexed File Editor**; never "cobolt". COBOL identifiers stay English. |
| **Fix vs feature** | **Feature** — new IDE capability + new `cobolt-forms` or sibling model crate + codegen; minor version bump + `CHANGELOG.md`. |
| **Code registry** | Add rows for editor/browser panels and `.cidx` model under `developers-guide-en.md` IDE sections in `/docsync`. |

## 7. Resolved decisions

| # | Decision |
|---|----------|
| **Q1** | **`.cidx`** extension, **XML** serialization (`.cfrm` patterns). |
| **Q2** | **One** generated COBOL file per definition: `generated/<stem>-indexed.cbl`. |
| **Q3** | Grid browser in a **separate OS window**. |
| **Q4** | Data paths **relativized** when under the project root; **absolute** when outside. |
| **Q5** | Unsupported composite/rich keys at finalize: **warn-only** (do not block). |
| **Q6** | Grid browser supports **add**, **edit**, and **delete** rows (`WRITE` / `REWRITE` / `DELETE`). |
| **Q7** | **Indexed Files** tree category sits **after Forms, before Common Code**. |
| **Q8** | **Variable-length** records (min/max) included in the New Indexed File wizard at v1. |
| **Q9** | Editor layout: **toolbar + structure + label + value** four-pane template (2026-06-16). |
| **Q10** | Property captions in a **separate middle pane**; values in the right pane (not a single 2-column grid). |

---

**Next step:** Spec and plan are implemented; run **`/docsync`** when English docs should reflect the four-pane layout screenshots.