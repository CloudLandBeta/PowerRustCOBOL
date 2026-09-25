# Tasks — PowerChat, Phase 1

- **Status:** Phase 1 done (1.70.197) — the operator's live run pending
- **Plan:** ./plan.md   **Date:** 2026-09-24

`features` branch. Tests print quantified results (GOLDEN RULE #7).

- [x] **T1 — Regen tool** — `crates/cobolt-ide/examples/powerchat_regen.rs`: for every form in
  the manifest, writes `generated/<form>.cbl` (`cobolt_codegen::generate`), the SideMenu
  `.menu.yaml` (`save_menu`) and the manifest's main-form seal.
- [x] **T2 — Project skeleton** — manifest, `indexed/*.cidx`, `COPYBOOKS/*`, README.
- [x] **T3 — File-handling COBOL** — open-or-create, COMMIT per change, the five files;
  proven by `test_powerchat_files.rs` as a console program.
- [x] **T4 — topics-form** — list, create (+ KB collection), select.
- [x] **T5 — settings-form** — KB location, model list, keys through 076.
- [x] **T6 — documents-form** — list, import, delete, refresh with the progress overlay.
- [x] **T7 — chat-form (main shell)** — SideMenu, chat, conversations, tokens.
- [x] **T8 — home-form** — the month's tokens and conversations.
- [x] **T9 — Compile test** — `powerchat_compiles.rs`: freshness, parse, semantics, menu hash,
  seal, no secrets.
- [x] **T10 — Docs and finalize** — Developer's Guide section on the example; CHANGELOG + `z`;
  the operator's run.

## Deviations from the plan (Phase 1)

- **T8 home-form folded into chat-form.** The month's tokens and conversations
  show in the chat's status line (`PC-STATUS`) rather than on a form of their
  own. One fewer form; same R42 figures.
- **No `indexed/*.cidx` or `COPYBOOKS/` for PowerChat's own files.** Their
  SELECT/FD live in each form's `<file-control>`/`<file-section>`. `.cidx`
  descriptions matter for the user's data the model queries (075), which is
  Phase 2.
- **T3** is proven by `crates/cobolt-ide/tests/powerchat_runs.rs`, which runs
  the real generated programs rather than a separate console copy of their
  file code.
- **Written around three runtime defects found while testing (for `fixes`):**
  1. `ACCEPT x FROM ENVIRONMENT "name"` is misparsed: `ENVIRONMENT` lexes as
     the division keyword, so the source falls back to `DATE` and the leftover
     tokens silently swallow the rest of the procedure. `rcrun check` reports
     OK. PowerChat uses `DISPLAY … UPON ENVIRONMENT-NAME` /
     `ACCEPT … FROM ENVIRONMENT-VALUE`.
  2. `COMMIT` after an `OPEN` that failed panics the interpreter
     (`indexed_disk.rs` "file open"): the unopened file is still committed.
  3. A control property whose value is all digits is `MOVE`d into a `PIC X`
     item right-justified, as if numeric. PowerChat prefixes its
     conversation row ids with `c`.
- **Ambiguity, documented rather than fixed:** a method call written as a
  statement straight after a `MOVE` is parsed as another receiving field.
  PowerChat writes every call statement as `MOVE X::M(...) TO WS-OK`, and the
  guide now warns about it.

# Phase 2

- [x] **P2-1 — Agent mesh** — MODELS capability fields; three agents; election; PLAN →
  WORK → COMPOSE; re-election on change. Test: `powerchat_runs` gains a two-agent
  mesh run against the scripted model (plan, parallel work, composed answer).
- [x] **P2-2 — Registered indexed files** (R20–R25)
- [x] **P2-3 — Prompt versions** (R48) — `prompts-form`; versions in `PROMPTS`, the
  active one's text kept in `TOP-PROMPT`; two-press confirmation (no message box in
  the runtime). **Not done:** 063 R45's user-dragged grip on the editor — it is a
  fixed-size multi-line TextBox.
- [x] **P2-4 — Sample topics installer** (Q1) — `samples/samples.txt` lists TOPIC / DOC /
  FILE lines; `topics-form` installs them (documents imported one at a time, chained by
  `onIndexed`) and removes them (topics flagged `TOP-SAMPLE`). The orders file is written by
  `powerchat_regen` when missing.
- [x] **P2-5 — Six languages** (R44–R46) — a translation table per form (FILLER rows,
  REDEFINES, one column per language) generated from one list; `PC-TEXTS` fills `T-` items
  and re-applies designed captions/hints; `PC-FMT` places `&1`..`&4`; six `PictureBox`
  flags (PNG, `assets/flags/`) in the SideMenu footer; menu rows moved to run time
  (designed rows cannot be relabelled); every window titled *PowerChat* (a title cannot
  change after the window opens).
- [x] **P2-6 — Documents folder tree** (R49) — `documents-form`'s ListBox is a
  TreeView built with `AddNode`. Folders are recorded in `folders.idx` (topic + path),
  so an empty one shows. The tree is the union of those folders and every folder
  that holds a document, sorted depth-first on a key where `/` sorts below
  everything. Selecting a folder (or a document in one) points the drop zone at it.
  **New folder** creates one inside the selection. **Delete** removes a document, or
  a folder only once nothing is under it. Test: `powerchat_runs` makes, fills,
  refuses, empties and deletes a folder.
  - **R50 deferred:** moving a document by dragging it needs TreeView drag and
    drop (spec 067, not built).
  - **Found on the way, fixed separately (1.70.207, `fixes`):**
    - a refused OPEN left the file open (41/48 after a 35);
    - INSPECT ignored reference modification;
    - `MOVE … TO T(I)(a:b)` dropped the subscript;
    - `"\"` was not a valid literal.

# Phase 3 — the operator's review (2026-09-25)

- [x] **P3-1 — First run** — with no agent assigned a model, the menu is shut
  but for RAG settings and Chat (the way home), and the chat shows a welcome
  screen: the name at 84 pt (6 × the form's 14), a robot (`assets/robot.svg`,
  chibi, the PowerRustCOBOL emblem on its chest) and four steps. Nothing is
  forced open any more.
- [x] **P3-2 — Everything embedded** — the five forms are `FormFormat` Embedded
  and open in the ContentPane from their menu rows (`open-form:`); their Close
  buttons are gone; each refreshes itself on `onActivate`, and so does the chat.
- [x] **P3-3 — The menu is designed** (`SideMenu-1.menu.yaml`), so the
  designer and the preview show it. Relabelled with `SetItemLabel` and held shut
  with `SetItemEnabled`, which reach designed rows since 1.70.210.
- [x] **P3-4 — RAG settings** — the Knowledge Base folder has a **…** button
  (`COBOL-FOLDER-DIALOG`, 1.70.211); agents are chosen in a ComboBox; the
  provider, model list and connection test are the IDE's (1.70.212); **Export…**
  / **Import…** write and read `rag-settings.xml` (no keys); every field has a
  tooltip; the status line is captioned **Status** on every form.
- **Found on the way, not fixed here:**
  - `SetSelectedIndex` on a ComboBox does not move its `Value`.
    PowerChat sets `Value` itself wherever it picks an item.
  - `ME::Close()` on an embedded form takes it off the pane without telling the
    shell. That is why the embedded forms have no Close button.
