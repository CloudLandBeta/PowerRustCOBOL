# Spec — SideMenu: run-time rows and hosted controls

- **Status:** draft → awaiting operator review
- **Folder:** specs/066-sidemenu-runtime-rows/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-22

> Feature `066` of the umbrella spec **063 — RAG / Transactional Chatbot
> boilerplate** (063 R36–R39, AC15). It is independent of every other 063
> feature and benefits any application that uses a SideMenu.

## 1. Overview

A SideMenu's rows are **static**. They live in a `<control-id>.menu.yaml`
sidecar beside the form, carry an HMAC hash, and every host reads that file once
at start-up (the designer through `set_menu_cache`, `rcrun run-form` in
`form_gui.rs`, the compiled binary from its embedded `MENUS` table). Nothing a
running COBOL program does can change them: no `AddItem`, `Clear` or `Items`
path reaches a SideMenu, and no `StateUpdate` feeds the menu data. The renderer
(`sidebar.rs`) and the running shell's MenuPane (`shell.rs`
`draw_mounted_menus`) both take their rows from the mounted `MenuDefinition`,
never from a property.

A chat application's sidebar is mostly *data*: the list of past conversations,
the documents in a folder, the files a user may consult. Those rows exist only
at run time. This feature lets a program add, change and remove SideMenu rows
while it runs, and lets controls (an assistant picker, a language picker) sit
in the rail — with identical behaviour on all four surfaces the renderer serves.

### What already exists and is reused

- **The footer Panel** (`sync_side_menu_footer_panels`): a SideMenu already owns
  a Panel `<side>-Footer`, pinned to its footer band, into which the developer
  drops ordinary controls. In a shell the rail draws that subtree
  (`host.rs:436-460`). That is hosted controls, restricted to one band.
- **The row model** (`sidebar::RowKind` — `Header | Section | Item | Footer`)
  and one `layout` / `paint` pair used by every surface.
- **The click path**: a leaf row pushes `SelectedItemId` and fires
  `onMenuItemClick`, on both `render_interactive` and the shell.

## 2. Goals / Non-goals

**Goals**

- A running program can add, relabel, badge, enable/disable and remove rows,
  and clear everything it added, without touching the designed rows.
- Run-time rows behave exactly like designed ones: same look, same selection
  highlight, same `onMenuItemClick` with the row's id in `SelectedItemId`,
  same collapse-to-rail behaviour.
- Controls can be placed in the rail above the footer band, not only inside it.
- One behaviour on the designer canvas, form preview, Run Form, and the
  running shell's MenuPane, in both the interpreter and the compiled binary.

**Non-goals**

- Editing the `.menu.yaml` from a running program. Run-time rows are never
  written back; the designed menu and its hash stay the developer's.
- Drag-and-drop reordering of menu rows.
- Run-time *navigation* actions (`open-form:`, `open-standalone-*`) on a row
  created at run time — see Q2.
- Changing MenuBar. It is a separate type and stays static.

## 3. User stories

- As a **developer**, I want to list the user's past conversations in the
  sidebar, so that history is part of navigation rather than a separate screen.
- As a **developer**, I want a combo box for the assistant and one for the
  language inside the rail, so that the whole application shell is the sidebar.
- As an **end user**, I want a new conversation to appear in the sidebar the
  moment I start it, and to disappear when I delete it.

## 4. Requirements (EARS)

### 4.1 Run-time rows

- **R1 (ubiquitous):** A running program shall be able to add a row to a
  SideMenu, giving it an id, a label, and optionally an icon, a badge and a
  parent row.
- **R2 (ubiquitous):** A running program shall be able to add a section title.
- **R3 (ubiquitous):** A running program shall be able to change the label,
  icon, badge and enabled state of any row it added, identified by id.
- **R4 (ubiquitous):** A running program shall be able to remove a row it added,
  by id, together with that row's children, and to remove every row it added in
  one call.
- **R5 (constraint):** A running program shall not remove or rename a row that
  came from the designed `.menu.yaml`. Such a call shall leave the menu
  unchanged and report failure to the caller rather than stopping the program.
- **R6 (ubiquitous):** Run-time rows shall appear after the designed rows, in
  the order they were added, unless added under a parent row, in which case they
  appear as that row's last children.
- **R7 (event):** When a run-time row is clicked, the SideMenu shall set
  `SelectedItemId` to the row's id and fire `onMenuItemClick`, exactly as for a
  designed row with the `event` action.
- **R8 (ubiquitous):** A row id shall be unique within the SideMenu. Adding a
  row whose id already exists shall replace the run-time row of that id in
  place, and shall fail for a designed row's id (R5).
- **R9 (ubiquitous):** A running program shall be able to ask how many run-time
  rows exist and whether a given id exists.
- **R10 (constraint):** The nesting limit of the designed menu (three levels)
  shall apply to run-time rows as well.

### 4.2 Hosted controls

- **R11 (ubiquitous):** A developer shall be able to place controls in a
  **header band** of the SideMenu (below the title, above the rows), in the
  same way the footer band already hosts them.
- **R12 (state):** While the SideMenu is collapsed to its rail, hosted controls
  in the header band shall be hidden, and shall reappear unchanged when it is
  expanded.
- **R13 (ubiquitous):** Controls hosted in a band shall receive events and
  property changes exactly as they do anywhere else on the form.

### 4.3 Parity

- **R14 (constraint):** Run-time rows and hosted controls shall render and
  respond identically on the designer canvas, form preview, Run Form, and the
  running shell's MenuPane. *(063 R39)*
- **R15 (constraint):** The behaviour shall be the same under `rcrun run-form`,
  embedded child forms, and the compiled binary.
- **R16 (ubiquitous):** The designer canvas shall show only designed rows — a
  run-time row exists only while a program is running.

## 5. Acceptance criteria

- [ ] **AC1** — A COBOL program adds 50 rows, a section and a nested row; all
      appear in order under the designed rows. *(R1, R2, R6)*
- [ ] **AC2** — Clicking a run-time row sets `SelectedItemId` to its id and runs
      the `onMenuItemClick` handler, on Run Form and in the shell. *(R7, R14)*
- [ ] **AC3** — Relabel, badge, disable and remove of a run-time row are visible
      on the next frame; removing a parent removes its children. *(R3, R4)*
- [ ] **AC4** — Attempting to remove or rename a designed row leaves the menu
      unchanged, returns a failure indicator, and the program continues. *(R5)*
- [ ] **AC5** — Clearing run-time rows restores exactly the designed menu. *(R4)*
- [ ] **AC6** — A ComboBox dropped into the header band is shown in the rail
      when expanded, hidden when collapsed, and fires its events. *(R11–R13)*
- [ ] **AC7** — The same rows, produced by the same program, lay out to the
      same row rectangles on `render_interactive` and on the shell's
      MenuPane (a test compares `sidebar::layout` inputs from both). *(R14)*
- [ ] **AC8** — A compiled binary shows the same run-time rows as
      `rcrun run-form` for the same program. *(R15)*
- [ ] **AC9** — `engine_reference_form_parity_static_vs_faces` still passes, and
      `cargo test -p cobolt-forms --features render` is green. *(R14, R16)*

## 6. Constraints & steering check

- **i18n:** row labels are the developer's own data and are not translated.
  The header band needs no new IDE string unless the designer grows a menu
  entry for it — if it does, that entry is a `Tr` field in all six languages.
- **Generated code:** no change to the generated-code contract is expected. If
  codegen emits a declaration for the header Panel, it follows the footer
  Panel's existing pattern.
- **System KB:** new SideMenu methods (and the header band) change a control's
  methods and behaviour, so the `cobolt-compiler` method/property doc tables
  and the SideMenu purpose text are updated and `assets/knowledge/chunked.data`
  regenerated in the same change. *(063 §6)*
- **Methods must be callable:** every new method name goes into
  `is_known_method`, or `Menu::AddRow(...)` parses as a subscript and silently
  does nothing.
- **Three hosts:** read the `interpreter-binary-parity` skill — the compiled
  binary's `run_form_app` must receive the same wiring as `rcrun run-form` and
  `host.rs`.
- **Docs:** `docs/developers-guide-en.md` gains a section on run-time rows and
  hosted bands. Under GOLDEN RULE #8 the Guide has only `-en.md` today, so no
  translation is deleted.
- **No self-resizing:** the header band has a fixed height the developer sets;
  its content never grows the SideMenu.
- **Fix vs feature:** a **feature** — run-time menu data and a new hosting band
  are capabilities beyond the control's existing scope. `features` branch,
  `z` bump.

## 7. Open questions

- **Q1 — the method surface.** Proposed, in COBOL call form:
  `AddRow(id, label [, icon [, parent-id]])`, `AddSection(title)`,
  `SetRowLabel(id, text)`, `SetRowIcon(id, icon)`, `SetRowBadge(id, text)`,
  `SetRowEnabled(id, flag)`, `RemoveRow(id)`, `ClearRows()`, `RowCount()`,
  `HasRow(id)`. *Recommendation:* these names, because the generic `AddItem`
  / `Clear` already mean "append a line to `Items`" on list controls and a
  SideMenu has no `Items` — reusing them would suggest a shape that is not
  there.
- **Q2 — navigation on run-time rows.** Designed rows can open forms
  (`open-form:`, `open-standalone-*`). *Recommendation:* run-time rows fire
  only `onMenuItemClick` in this feature; the handler opens whatever it needs
  with the existing COBOL form-opening calls. Adding an action argument later
  stays compatible.
- **Q3 — the header band vs. controls inside rows.** The umbrella asks for
  controls "in the rail". *Recommendation:* a header band mirroring the
  existing footer Panel (R11), rather than a row type that embeds a control —
  it reuses a proven mechanism on every surface, and the assistant and
  language pickers are both fixed-position controls. A per-row hosted control
  can follow if an application needs one.
- **Q4 — transport.** How run-time rows travel from the interpreter to the
  renderer and the shell is a `/plan` decision. The constraint it must honour:
  the shell and `render_interactive` read the *same* merged row list, so AC7
  can compare them.
