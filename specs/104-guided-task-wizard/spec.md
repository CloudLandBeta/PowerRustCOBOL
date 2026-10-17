# Spec — Task Wizard

- **Status:** **approved → ready for `/plan`** (clarified 2026-10-17: the operator decided four questions; the rest were settled by the author under the operator's standing instruction, "do what is right for the project in the long term, not what is easier or less risky" — each one recorded in §9)
- **Folder:** specs/104-guided-task-wizard/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-17

## 1. Overview

A **wizard of the Tour kind**. A toggle button — on the **IDE toolbar** and on the
**Form Designer (RAD) toolbar**, one switch behind both — turns the **Task Wizard** on
and off. While it is on, the developer can start a **task**: a guided flow that takes
them through doing one thing the IDE has a lot of settings for — configuring a model
provider, connecting an agent to a model, binding a control to data, setting up a
DataGrid, filling a TreeView, building a SideMenu or a menu, adding an animation — and
the list is meant to grow.

It looks like the IDE Walkthrough (spec 059): the rest of the window dims, one component
stays lit, and a comic-style speech balloon points at it and says what to do there. It
differs in what it is *for*. The Walkthrough shows **what is where** and is read; the
Task Wizard shows **how to get something done** and is *performed*: the lit component is
live, the developer operates it, and the step is complete when they have — not when they
press a button. Where a step is mechanical, the developer can let the wizard do it.

It exists because the IDE's configuration surfaces are deep. The Developer's Guide
explains each one, and the Walkthrough names the six main components, but nothing takes a
developer who has just decided "I need my agent to answer from a real model" from the
first click to a working result. Today they have to know that a *provider* is configured
in one manager, the *model* chosen in another, and that the second only lists what the
first has made available.

## 2. Goals / Non-goals

**Goals**

- One on/off switch, on both toolbars and in the Help menu, always showing the same
  state, remembered across restarts.
- A **catalogue of tasks**, opened from the switch, each shown as new, in progress or
  done.
- Each task is a sequence of steps; each step lights the real component to use, explains
  in a balloon what to put there, and **moves on by itself when the developer has done
  it**.
- **Do this for me** on the steps that are mechanical, from the first release.
- It never traps: Next, Back and Exit are always there, Exit remembers where the
  developer was, and a closed window or a moved target never leaves a balloon pointing at
  nothing.
- The look is the Walkthrough's — same dim, same balloon, same palette — so the two read
  as one family.
- Adding a task over surfaces the wizard already knows is **adding a definition**, not
  writing engine code.
- **All eight tasks of §3 in the first delivery.**

**Non-goals**

- **Replacing the Walkthrough.** The Tour stays as spec 059 made it: six steps, read-only,
  once per machine. This feature does not change it, and does not add a nudge to it.
- **Replacing the Developer's Guide.** A balloon is a sentence or two; depth stays in the
  Guide.
- **An AI assistant.** The Task Wizard is scripted and deterministic. It calls no model
  and no network, and is unrelated to Grace and to the Claude Code integration.
- **A dialog wizard.** The IDE already has modal, multi-page wizards (*Connect a coding
  agent*, spec 084; *New Indexed File*; the AI setup wizard of spec 040, R11). This is not
  another one: it guides the developer through the IDE's *own* editors and never replaces
  them with a form of its own.
- **Handling secrets** (R28).
- **Re-enabling what the IDE has deliberately disabled.** Creating and deleting agents is
  switched off in the Agents Manager (`SHOW_NEW_AGENT_CONTROL`, pinned by a test); the
  agent task connects an existing agent to a model instead.
- **Offers on every control.** Offers are for the four controls a task is *about* (R10).
- **Branching tasks.** A task is a linear list of steps; a step that does not apply is
  skipped, not branched around.

## 3. Vocabulary and the eight tasks

- **Task Wizard** — the feature and its switch. On: the catalogue and the offers are
  available. Off: nothing of it is on screen and it costs nothing (R35).
- **Task** — one thing a developer wants done, with a beginning and an end.
- **Step** — one anchor, one balloon, one **completion condition**.
- **Anchor** — the widget or section a step is about. The **ring** is drawn around it.
- **Live region** — what the developer may operate during the step: the anchor's
  *container* — the whole window that holds it, or the scrolling pane for a row of the
  Properties pane — so the window's own Save, tabs and scrollbar stay usable. Tree,
  toolbar and canvas anchors have a tight live region.
- **Via** — the control that (re)opens a step's container: the button that opens the
  window, the tab that shows the section.
- **Subject** — the control a task is about, captured when the developer selects it.
- **Mechanical step** — one whose action is the same for everyone and safe to automate;
  only these offer **Do this for me** (R25–R27).
- **Offer** — a small, dismissible suggestion, shown only while the Task Wizard is on,
  that a task fits the control the developer has just selected. An offer starts nothing
  by itself.

The eight tasks are the operator's list (2026-10-17: *models, agents, databound controls,
datagrids, treeviews, sidemenus, menus, animations, etc.*), mapped onto the surfaces the
code actually has:

| # | Task | Where it happens | What the developer ends with |
|---|---|---|---|
| 1 | **Configure a model provider** | Project Settings → **Model Providers…** → the Model Providers Manager (spec 048, `panels/models_modal.rs`) | A provider with its endpoint and key, tested and saved, that any agent can use |
| 2 | **Connect an agent to a model** | Project Settings → **Manage agents…** → the Agents Manager's *Agent × Model* tab (spec 028, `panels/agents_modal.rs`) | An existing agent running on a chosen provider and model |
| 3 | **Bind a control to data** | The **Data Binding** section of the Properties pane and its editor window (`panels/data_binding.rs`, `properties.rs`) | A bindable control bound to a field of an indexed file, SQL source, COBOL table or REST source |
| 4 | **Set up a DataGrid** | Its **Data Binding**, then **Edit DataGrid settings…** (the DataGrid settings window); an unbound grid gets its columns in the canvas's grid-edit mode | A grid on its data, with columns, widths and behaviour set |
| 5 | **Fill a TreeView** | The inline **Items** field of the TreeView section of the Properties pane, and its behaviour rows | A TreeView with its own nodes |
| 6 | **Build a SideMenu** | **Edit Menu…** → the menu editor, opened for a side menu (its items live in `<control id>.menu.yaml`) | A rail of items that open forms or run actions |
| 7 | **Build a menu** | **Edit Menu…** → the menu editor, opened for a MenuBar | A MenuBar with items, shortcuts and handlers |
| 8 | **Add an animation** | The **Anim** tab of the Properties pane (`show_animations`) | An animation with trigger, kind and repeat, previewed |
| 9 | **Set up a map** *(added 2026-10-17, 1.91.196)* | The Maps section of the Properties pane — *Start latitude/longitude*, *Start zoom*, its **🎨 Colors** — its **Data Binding** for pins, and the **Events** tab | A Maps control that opens where and how close the developer chose, optionally with a pin per row of a table, and its events wired |

The `data_grid_columns.rs` Column Editor is not a surface: it is not wired into the IDE.

The text of each step is written with the operator in the six languages; this spec fixes
the mechanism and the subjects.

## 4. User stories

- As a developer who has never configured an AI provider, I want to be led through it on
  the real screens, so that I end with one that works instead of a half-filled window and
  a trip to the manual.
- As a developer who knows the IDE, I want to leave the Task Wizard off and never see it.
- As a developer binding a grid for the first time, I want the wizard to light the next
  control while I operate it myself, so that I learn where it is.
- As a developer who was interrupted halfway through a task, I want to come back to the
  step I was on, in the project I was in.
- As a developer who finds a step tedious, I want the wizard to do it for me, and to undo
  it as I would undo my own edit.
- As a developer who works in six languages, I want the balloons in mine.
- As a maintainer, I want to add a task by writing its steps, so that the IDE can teach
  each new feature without a new mechanism.

## 5. Requirements (EARS)

### The switch

- **R1 (ubiquitous):** The IDE toolbar shall carry the Task Wizard toggle at its **right
  end, beside the language selector**, and the Form Designer toolbar shall carry it **at
  the end of its group of view toggles, after the Magnifier**. Both shall show, and
  change, **the same single state**.
- **R2 (state):** While the Task Wizard is on, both buttons shall read as **engaged**, as
  the Designer's other toggles do (Magnifier, Show Grid), in every IDE theme, including
  the pill buttons of Aurora Pastel.
- **R3 (ubiquitous):** Each toggle shall be a **split button**: the toggle itself, and a
  **▾** beside it that always works. Turning the Task Wizard on shall open the
  **catalogue** in a popup under the button that was pressed; the ▾ shall reopen it, and
  while the Task Wizard is off the ▾ shall turn it on, which opens the catalogue.
  *Amended 2026-10-17 (1.91.197):* the ▾ was live only while on, so with the wizard off
  it did nothing and read as broken (operator: "if the switch is not selected the arrow
  does not work, but then it should select the switch and open the options").
- **R4 (event):** When the toggle is pressed while the Task Wizard is on, it shall turn
  off: a running task shall stop, **keeping its progress**, and the catalogue and the
  offers shall go.
- **R5 (ubiquitous):** The state shall be **machine-level**, stored with the developer's
  other interface preferences (`UiPrefs`, `ui.toml`), not in a project, and shall survive
  a restart. It shall be **off** on a machine that has never set it.
- **R6 (ubiquitous):** The Help menu shall carry the same switch as a checkable item,
  after *IDE Walkthrough*. It belongs in Help, not in the Settings menu, for the reason
  spec 099 Q1 gives for the Walkthrough's own item: it switches a way of working on and
  off and configures nothing. Turning it on there opens the catalogue under the IDE
  toolbar's button.
- **R7 (state):** While no project is open, the Task Wizard may be switched on, and every
  task shall be shown disabled **with the reason** (all eight need a project), never
  started and never broken.

### Offering tasks

- **R8 (event):** When the catalogue opens, it shall list every task with its state: *new*,
  *in progress* (for the open project), or *done*.
- **R9 (ubiquitous):** Any task shall be startable from the catalogue, restartable when
  done, and resumable when in progress in the open project.
- **R10 (event):** While the Task Wizard is on, when the developer selects **exactly one**
  DataGrid, TreeView, SideMenu, MenuBar or Maps control in the Form Designer, the wizard shall show an
  **offer** beside it for the task about that control — unless that task is done, or the
  offer was dismissed for that control. An offer shall never start a task by itself, and
  no other control shall get one.
- **R11 (state):** While the Task Wizard is off, no offer shall be drawn, no catalogue
  shall open and no step shall run.

### Running a task

- **R12 (ubiquitous):** Each step shall dim the IDE except its live region, draw the
  **ring** around its anchor and a **speech balloon** whose tail points at it, and carry
  the task's title, "step *n* of *m*", the step's text, and **Back**, **Next** and
  **Exit** — plus **Do this for me** on a mechanical step.
- **R13 (state):** The **live region** shall be **live**: pointer input inside it shall reach
  the IDE, and keyboard input shall reach the widget that has focus inside it. Pointer
  input **outside** every hole of R16 shall not reach the IDE beneath, so a stray click
  cannot wander off the task. *(This is the deliberate difference from the Walkthrough,
  whose R4 and R6 keep the tour read-only.)*
- **R14 (ubiquitous):** Anything the live region opens — a combo box's list, a menu, a
  colour picker, a tooltip — shall be live while it is open, even where it hangs over the
  dim.
- **R15 (ubiquitous):** The dim shall cover every other part of the IDE, other windows
  included. The Form Designer is its own OS window: a viewport that does not hold the
  anchor shall be dimmed and blocked whole, except its Task Wizard toggle, and shall show
  a chip that names where the task continues and brings that window forward.
- **R16 (ubiquitous):** The holes in the dim shall be: the live region; both Task Wizard
  toggles and their ▾ (so the wizard can always be turned off); the balloon; and any
  window that **opened after the step began** (an error, a confirmation), so nothing the
  IDE asks the developer is ever hidden behind the wizard.
- **R17 (ubiquitous):** Each step shall declare a **completion condition** read from the
  IDE's own state. The wizard shall advance by itself only when the condition becomes
  true **while the step is showing**. A step whose condition already holds when it is
  entered shall say so ("already done") and make **Next** its primary button; going
  **Back** to a finished step shall never bounce forward on its own.
- **R18 (ubiquitous):** **Next** shall always be available, whether or not the condition
  holds. A step left with its condition unmet shall be listed in the closing summary as
  skipped.
- **R19 (ubiquitous):** A task's steps shall run in order. A step whose precondition shows
  it does not apply (a provider that takes no key, a grid that is already bound) shall be
  skipped, and "*n* of *m*" shall count only the steps that apply.
- **R20 (state):** While a step's anchor is not on screen — a window closed, a tab switched,
  a section scrolled away — the wizard shall **not point at nothing**. In order: if the
  anchor is elsewhere on screen, follow it; if it is scrolled out of view, scroll it in;
  if its container is closed, light the step's **via** with "Reopen …"; failing that, go
  back to the nearest earlier step whose anchor is on screen.
- **R21 (event):** When a task finishes, the wizard shall show a **summary**: what was done,
  what was skipped, and where the result lives.
- **R22 (event):** **Exit**, turning the Task Wizard off, closing the project and closing the
  IDE shall all leave the task *in progress at its current step*, for that project. `Esc`
  shall leave the innermost thing first: when a field has focus, `Esc` is the field's own
  (a DataGrid cell editor cancels its edit); when a popup is open, `Esc` only closes the
  popup; when the window is in a mode `Esc` leaves (a grid's edit mode on the canvas), `Esc`
  only leaves the mode; otherwise `Esc` is **Exit**. One press never does two of these.
  *(Amended after the DataGrid review, 2026-10-17: swallowing a field's `Esc` turned a cell
  editor's cancel into a commit, and taking the second `Esc` hid spec 094's "Esc leaves the
  mode".)*
- **R23 (state):** While a task runs, keyboard input shall reach only a widget that has
  focus inside the live region; focus elsewhere shall be released, and the IDE's own
  shortcuts (open, save, run, delete, undo on the canvas, tree navigation) shall not act.
  A step whose anchor is the canvas may let keys reach the whole Designer window.
- **R24 (state):** The wizard shall **pause** — draw nothing and take no input — while the
  Walkthrough runs, while a modal dialog owns the screen (the Building dialog, a
  confirmation), and while the anchor's window is hidden behind another application. It
  shall resume where it was. While the Walkthrough runs, no task can start.

### Do this for me

- **R25 (ubiquitous):** Every step the task marks **mechanical** shall offer **Do this for me**
  from the first release. It shall do exactly what the developer would do by hand, through
  the same code path: in the Form Designer as **one undo entry**; in a manager window as a
  change to the manager's own draft, which its Cancel discards. It shall say what it did.
- **R26 (constraint):** The wizard shall never overwrite or delete anything the developer
  wrote. **Do this for me** shall be offered only while the field or list it fills is empty.
  *(User code is sacred.)*
- **R27 (constraint):** The wizard shall never call a model or the network, never build,
  and never run the application. No step whose action would — opening the Model Providers
  Manager, selecting a provider, Test connection (each of which fetches or connects) — nor
  any step in a surface that saves with no undo (the Agents Manager's *Agent × Model* tab),
  nor any step that writes the developer's own content (a TreeView's nodes), shall be
  mechanical.

### Secrets

- **R28 (constraint):** The wizard shall never enter, read, show, log or store an API key or
  other secret. The state it reads (its *facts*) shall carry **nothing** about a secret
  field — not its value, not its length, not whether it is empty. A step that asks for a
  key shall complete on a non-secret signal: the provider's connection test passed, or the
  provider became configured on Save.

### Memory

- **R29 (ubiquitous):** A task in progress shall be remembered **per project**, by the **key**
  of its current step (never its position, which changes when a task is edited); a task
  **done** shall be remembered **per machine** (it records what the developer has learned).
  Both shall be machine-level, beside the switch (R5), and survive a restart.
- **R30 (ubiquitous):** A dismissed offer shall be remembered by project, form, control and
  task, and forgotten when that control no longer exists in an open form.
- **R31 (constraint):** The wizard shall write nothing into a project folder for its own
  bookkeeping.

### Fit with the IDE

- **R32 (ubiquitous):** Every string the wizard shows — toolbar tips, the menu item, the
  catalogue, every step's title and text, buttons, summaries, reasons, offers, chips —
  shall be a `Tr` field translated in all six languages (EN/ES/PT/JA/ZH/FR).
- **R33 (ubiquitous):** The wizard shall follow the active IDE theme and stay legible on all
  32 themes, light and dark.
- **R34 (constraint):** The wizard shall **never resize any window** (`CONVENTIONS.md`). The
  balloon is sized from explicit numbers and placed by a pure function, never from the
  space it was handed.
- **R35 (constraint):** While the Task Wizard is off it shall draw nothing, record no anchor,
  allocate nothing and create no layer; each place that can report an anchor shall cost one
  check.
- **R36 (ubiquitous):** The wizard's logic — which step shows, whether its condition holds,
  where the dim, the holes and the balloon go — shall be **testable without the application
  object**, which cannot be built in a test. It shall be handed what the frame painted and a
  read-only snapshot of the IDE's state, and shall return what it wants drawn and done.
- **R37 (ubiquitous):** A task shall be **a declarative definition** — its steps, each with an
  anchor, its text, a completion condition, an optional via, an optional precondition and an
  optional mechanical action — held in one table. Adding a task over surfaces the wizard
  already knows shall not change the engine.
- **R38 (constraint):** This feature shall **not change the Walkthrough's behaviour**: every
  acceptance criterion of spec 059 keeps passing, and the Walkthrough paints exactly what it
  painted before.

## 6. Acceptance criteria

- [ ] **AC1** — Both toolbars carry the split toggle in the place R1 gives. Pressing either
      changes both; the state survives an IDE restart and is **off** on a fresh `ui.toml`.
- [ ] **AC2** — Engaged on both toolbars on all 32 themes, Aurora Pastel's pills included
      (asserted on the painted button, not on a predicted colour).
- [ ] **AC3** — Turning it on opens the catalogue under the pressed button; ▾ reopens it,
      and with the wizard off one click on ▾ turns it on and opens the catalogue, in the IDE
      toolbar and in a Form Designer window; the Help menu's item mirrors the state, changes
      it, and opens the catalogue when it turns it on.
- [ ] **AC4** — With no project open the switch turns on, and every task is disabled with its
      reason and cannot start.
- [ ] **AC5** — The catalogue lists the eight tasks of §3 with the right state, and each can be
      started, resumed after Exit and restarted when done.
- [ ] **AC6** — For **every step of every task**, the ring equals the rect of the anchor as the
      frame actually painted it, and the balloon's tail touches it.
- [ ] **AC7** — A click inside the live region reaches the widget under it; a click on the dim
      reaches nothing; the mouse wheel over the dim scrolls nothing; text typed into a
      focused field in the live region arrives.
- [ ] **AC8** — A combo box list opened from the live region and hanging over the dim can be
      picked from.
- [ ] **AC9** — A non-target window under the dim takes no focus and no click; a window that
      opens during a step is not dimmed and answers clicks.
- [ ] **AC10** — With the anchor in the Form Designer, the main window is dimmed and blocked
      except its toggle, and shows the "continue in" chip (and vice versa).
- [ ] **AC11** — A step whose condition becomes true while it shows advances by itself; one
      whose condition already held at entry says "already done" and waits for Next; Back to
      a finished step does not bounce forward; **Next** always advances; a skipped step is in
      the summary; "*n* of *m*" counts only steps that apply.
- [ ] **AC12** — Closing the anchor's window mid-step lights its via with "Reopen …"; no balloon
      ever points at empty space.
- [ ] **AC13** — **Exit**, the toggle, and closing the project leave the task in progress at the
      same step for that project; another project shows it as not started; the catalogue
      resumes it in the first.
- [ ] **AC14** — `Esc` with a field focused leaves the field; with a popup open closes only the
      popup; otherwise exits — one stage per press.
- [ ] **AC15** — While a task runs, the IDE's shortcuts and the project tree's keys do nothing,
      and focus outside the live region is released.
- [ ] **AC16** — While the Walkthrough or a modal dialog runs, the wizard draws nothing and
      resumes after it; no task starts while the Walkthrough runs.
- [ ] **AC17** — **Do this for me** on each mechanical step produces the same change as the
      manual route; in the Form Designer it is undone by exactly one undo; it is not offered
      while its field or list holds a value.
- [ ] **AC18** — Completing each Form Designer task through the wizard yields the same `.cfrm`
      (and `.menu.yaml`) as the same task done by hand.
- [ ] **AC19** — No step that would reach the network, build, run, save without undo or write
      the developer's content is mechanical (asserted over the task table).
- [ ] **AC20** — A recognisable key typed into the real key field appears nowhere in `ui.toml`,
      the logs, or the `Debug` of the wizard's facts and state; the facts type has no field
      derived from a secret field.
- [ ] **AC21** — Progress is stored by step key; a dismissed offer stays dismissed after a
      restart and is forgotten when its control is deleted.
- [ ] **AC22** — Every string exists in all six languages, the i18n tests pass, and a rendered
      pass over the catalogue and one balloon of each task shows **no word left in English**
      in the other five.
- [ ] **AC23** — Balloon text and its backdrop meet the contrast the IDE already requires,
      measured on the **painted** colours, on all 32 themes.
- [ ] **AC24** — No window changes size during any task, measured over rendered frames in all
      six languages.
- [ ] **AC25** — With the Task Wizard off, no anchor sink is installed and no layer of the
      wizard exists over many frames (asserted by counters, not by timing).
- [ ] **AC26** — A fixture task defined in a test, as a definition alone, appears in the
      catalogue and runs, with no change to the engine.
- [ ] **AC27** — The Walkthrough paints, step by step, exactly the shapes it painted before this
      feature, and every acceptance criterion of spec 059 still passes.

## 7. Constraints & steering check

- **i18n (6 languages):** **yes, and large** — eight tasks of four to seven steps, each with
  a title and a text, plus the catalogue, buttons, summaries, reasons, offers and chips: well
  over a hundred `Tr` fields × six languages. No literal in a panel (`ui_text` source scans
  enforce it).
- **Generated-code contract:** none directly — the wizard writes no COBOL. The editors it
  drives do, exactly as before (R25, AC18).
- **System KB:** **no** — no control, property, method or event is added or changed. If any
  control documentation changes during implementation, `chunked.data` is regenerated in the
  same change.
- **Docs:** **yes** — `docs/developers-guide-en.md` gains a *Task Wizard* section. Only the
  English Guide exists, so no translation is touched (GOLDEN RULE #8).
- **Fix vs feature:** **feature** — a new IDE capability. A `features-<slug>` branch off
  `main` (another checkout holds `features`); `z` bump only, the minor is the operator's;
  announced on f=96 only if the operator asks.
- **DataGrid guardrail:** the wizard reports anchors from DataGrid code (grid-edit toolbar,
  the DataGrid section). Before completion, the DataGrid Quality & Compatibility review
  (`AGENTS.md`) is run, though no DataGrid behaviour changes.
- **Themes:** legible on all 32; hardcoded contrast where `ui.visuals()` cannot be trusted,
  as the Walkthrough and the chat balloons do.
- **A window may never resize itself:** R34, AC24.
- **Privacy:** R27, R28, AC19, AC20 — no secrets, no network, no telemetry.
- **Tests:** the engine is pure (R36) so that it can be tested; the app object cannot be
  constructed in a test and the project does not drive the application's window.

## 8. Decisions already taken

From the operator's request (2026-10-17):

- **The wizard is of the Tour type** — the dim, the lit target and the comic balloon of spec 059.
- **It is switched on and off by a toggle button** on the IDE toolbar **and** on the Form
  Designer (RAD) toolbar.
- **The first subjects** are models, agents, databound controls, DataGrids, TreeViews,
  SideMenus, menus and animations, and the list is open ("etc.").

From `/clarify` (operator, 2026-10-17):

- **Do this for me from the first release** (R25), not in a later phase.
- **All eight tasks in the first delivery** — no waves.
- **The catalogue opens in a popup under the button**; the button sits at the right end of
  the IDE toolbar beside the language selector, and at the end of the Designer's view
  toggles; **offers only on DataGrid, TreeView, SideMenu and MenuBar** (R1, R3, R10).
  *Amended 2026-10-17 (operator: "add a wizard for the map"):* the Maps control got a task
  of its own (task 9), so it gets the offer too — the rule was always "the controls a task
  is about", and the map is now one of them.
- **In progress per project, done per machine**, both kept in the developer's preferences,
  nothing in the project folder (R29, R31).

## 9. Settled by the author, and easy to reverse

Design calls inside the scope the operator set. The ones marked ✱ were made after
`/clarify`, while the operator was away, under the instruction "do what is right for the
project in the long term, not what is easier or less risky".

- **One shared, machine-level switch.** Two buttons with two states would disagree the moment
  the developer used both. It is stored with `UiPrefs`, beside `walkthrough_shown`, for spec
  059's reason: a way of working belongs to the developer, not to one project.
- **The live region is live.** A task cannot be done on a component the developer may not
  touch. The wizard keeps the Walkthrough's other half — everything outside is blocked.
- ✱ **The live region is the anchor's container, the ring is the anchor.** A step inside the
  Model Providers Manager needs its Save button and scrollbar; lighting only one field would
  make the step impossible to finish. The ring still shows exactly where to look.
- ✱ **Completion is an edge, seen while the step shows.** "True now" would make Back bounce
  forward at once and would skip a step whose field the IDE pre-fills (the endpoint).
- **Next is always there.** A wizard that waits for an event that never comes traps the
  developer; observing state is what makes it a wizard, and Next is what keeps it from being
  a trap.
- ✱ **A way back, not a dead end.** When the developer closes the window a step needs, the
  wizard lights the button that reopens it (R20) instead of parking a "Continue" chip that
  leaves only Exit.
- ✱ **The wizard knows nothing about secrets** (R28) — not even whether a key was typed. A
  boolean is small, but it is still information derived from a secret field, and the rule is
  easier to keep, and to prove, as "nothing".
- ✱ **Connect an agent to a model**, not *create an agent*: creation is deliberately disabled
  in the IDE, and a wizard must not reopen a door the product closed.
- ✱ **No Do this for me where it would be wrong** (R27): opening the providers manager and
  selecting a provider both fetch model lists over the network; the Agents Manager's *Agent ×
  Model* tab saves on every change with no undo; a TreeView's nodes are the developer's
  content.
- ✱ **Progress by step key, dismissals by project.** An index silently points at a different
  step once a task is edited; a dismissal keyed without the project collides across projects.
- ✱ **Pause, don't fight.** When the Building dialog or a confirmation owns the screen, the
  wizard steps aside (R24) rather than layering a second modal over the first.
- ✱ **The name is "Task Wizard"** in the interface. The IDE already has dialog wizards; the
  word *task* says what this one is about.
- **Default off.** A machine that has never set the switch behaves exactly as the IDE did
  before this feature.
- **The Walkthrough is left alone** — not amended, not nudging towards the wizard. It describes
  the IDE once; the Task Wizard teaches tasks, repeatedly, on demand.

## 10. Open questions

*(None. The spec is ready for `/plan`.)*

Recorded assumptions, easy to change at review: the toggle's glyph and icon are chosen during
implementation from glyphs the bundled fonts are proven to contain; the step texts are written
by the author and reviewed by the operator in the running IDE.
