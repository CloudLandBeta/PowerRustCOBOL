# Spec — Guided Task Wizard

- **Status:** draft → **needs `/clarify`** (seven open questions in §10, each with a recommendation; none blocks the shape of the feature)
- **Folder:** specs/104-guided-task-wizard/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-17

## 1. Overview

A **Wizard** of the Tour kind. A toggle button — on the **IDE toolbar** and on the
**Form Designer (RAD) toolbar**, one switch behind both — turns **Wizard mode** on
and off. While it is on, the developer can start a **task**: a guided flow that
takes them through doing one thing the IDE has a lot of settings for — configuring
a model provider, creating an agent, binding a control to data, setting up a
DataGrid, filling a TreeView, building a SideMenu or a menu, adding an animation —
and the list is meant to grow.

It looks like the IDE Walkthrough (spec 059): the rest of the window dims, one
component stays lit, and a comic-style speech balloon points at it and says what to
do there. It differs in what it is *for*. The Walkthrough shows **what is where**
and is read; the Wizard shows **how to get something done** and is *performed*: the
lit component is live, the developer operates it, and the step is complete when
they have — not when they press a button.

It exists because the IDE's configuration surfaces are deep. The Developer's Guide
explains each one, and the Walkthrough names the six main components, but nothing
takes a developer who has just decided "I need an agent that answers from my
knowledge base" from the first click to a working agent. Today they have to know
that a *provider* is configured in one manager, the *agent* in another, and the
model chosen in the second, not the first.

## 2. Goals / Non-goals

**Goals**

- One on/off switch for Wizard mode, on both toolbars, always showing the same
  state, remembered across restarts.
- A **catalogue of tasks**, opened from the switch, each shown with whether it is
  new, in progress or done.
- Each task is a sequence of steps; each step lights the real component to use,
  explains in a balloon what to put there, and **moves on by itself when the
  developer has done it**.
- It never traps: Next, Back and Exit are always there, Exit remembers where the
  developer was, and a closed window or a moved target never leaves a balloon
  pointing at nothing.
- The look is the Walkthrough's — same dim, same balloon, same palette — so the two
  read as one family.
- Adding a task is **adding a definition**, not writing engine code.

**Non-goals**

- **Replacing the Walkthrough.** The Tour stays as spec 059 made it: six steps,
  read-only, once per machine. This feature does not change it (§8, §9).
- **Replacing the Developer's Guide.** A balloon is a sentence or two; depth stays
  in the Guide.
- **An AI assistant.** The Wizard is scripted and deterministic. It calls no model
  and no network, and is unrelated to Grace and to the Claude Code integration.
- **A dialog wizard.** The IDE already has modal, multi-page wizards (*Connect a
  coding agent*, spec 084; *New Indexed File*; the AI setup wizard of spec 040, R11).
  This is not another one: it guides the developer through the IDE's *own*
  editors and never replaces them with a form of its own.
- **Handling secrets.** The Wizard never reads, shows, logs or stores an API key;
  the developer types it into the real field.
- **Covering every feature in the first release.** §3 names the first tasks; the
  mechanism is what must be general.

## 3. Vocabulary and the first tasks

- **Wizard mode** — the switch. On: the catalogue and the offers are available. Off:
  nothing of the Wizard is on screen and it costs nothing.
- **Task** — one thing a developer wants done, with a beginning and an end.
- **Step** — one lit target, one balloon, one **completion condition**.
- **Offer** — a small, dismissible suggestion, shown only while Wizard mode is on,
  that a task fits what the developer has just selected ("Set up this DataGrid?").
  An offer starts nothing by itself.

The first tasks are the operator's list (2026-10-17: *models, agents, databound
controls, datagrids, treeviews, sidemenus, menus, animations, etc.*). Where each
happens today. Every surface below was found in the code; the two marked *to be
confirmed* are the ones `/plan` must pin before it commits to an anchor:

| # | Task | Where it happens | What the developer ends with |
|---|---|---|---|
| 1 | **Configure a model provider** | Model Providers Manager (spec 048, `panels/models_modal.rs`) | A provider with its endpoint and key, tested, that any agent can use |
| 2 | **Create an agent** | Agents Manager (spec 028, `panels/agents_modal.rs`) | An agent on a chosen provider and model, with its limits |
| 3 | **Bind a control to data** | The Properties pane's data-binding section (`panels/data_binding.rs`) | A control bound to a field of an indexed file, SQL source or REST source |
| 4 | **Set up a DataGrid** | The DataGrid Column Editor (`panels/data_grid_columns.rs`) and its data source; write-back arrived in 1.91.15 and spec 095 continues it | A grid on a data source with its columns, widths and filters |
| 5 | **Fill a TreeView** | The TreeView section of the Properties pane (`panels/properties.rs`) — *to be confirmed: which editor holds the nodes* | A TreeView with nodes and their behaviour |
| 6 | **Build a SideMenu** | A SideMenu's items live in its `<control id>.menu.yaml` sidecar — *to be confirmed: whether the menu editor below is the one that edits them* | A rail of items that open forms or run actions |
| 7 | **Build a menu** | The menu editor the Properties pane opens (`show_menu_editor` in `panels/designer.rs`) | A MenuBar with items, shortcuts and handlers |
| 8 | **Add an animation** | The Animations section of the Properties pane (`show_animations`) | An animation with trigger, kind and repeat, previewed |

The text of each step is written in `/plan`, with the operator; this spec fixes the
mechanism and the subjects.

## 4. User stories

- As a developer who has never configured an agent, I want to be led through it on
  the real screens, so that I end with one that works instead of a half-filled
  modal and a trip to the manual.
- As a developer who knows the IDE, I want to leave Wizard mode off and never see it.
- As a developer binding a grid for the first time, I want the wizard to light the
  next control while I operate it myself, so that I learn where it is.
- As a developer who was interrupted halfway through a task, I want to come back to
  the step I was on.
- As a developer who works in six languages, I want the balloons in mine.
- As a maintainer, I want to add a task by writing its steps, so that the IDE can
  teach each new feature without a new mechanism.

## 5. Requirements (EARS)

### The switch

- **R1 (ubiquitous):** The IDE toolbar and the Form Designer toolbar shall each carry
  a **Wizard toggle button**. Both shall show, and change, **the same single
  state**.
- **R2 (state):** While Wizard mode is on, both buttons shall read as **engaged**,
  as the Designer's other toggles do (Magnifier, Show Grid), in every IDE theme,
  including the pill buttons of Aurora Pastel.
- **R3 (event):** When the button is pressed with Wizard mode on, Wizard mode shall
  turn off: a running task shall stop, **keeping its progress**, and the catalogue
  and the offers shall go.
- **R4 (ubiquitous):** The state shall be **machine-level**, stored with the
  developer's other interface preferences (`UiPrefs`), not in the project file, and
  shall survive a restart. It shall be **off** on a machine that has never set it.
- **R5 (ubiquitous):** The Help menu shall carry the same switch as a checkable item,
  so it can be reached when the Designer toolbar is not showing. It belongs in Help,
  not in the Settings menu, for the reason spec 099 Q1 gives for the Walkthrough's own
  item: it switches a way of working on and off and configures nothing.
- **R6 (state):** While no project is open, Wizard mode may be switched on, and every
  task that needs a project shall be shown disabled **with the reason**, never
  started and never broken.

### Offering tasks

- **R7 (event):** When Wizard mode is on and the developer opens the catalogue, the
  Wizard shall list every task with its state: *new*, *in progress at step n*, or
  *done*.
- **R8 (ubiquitous):** Any task shall be startable from the catalogue, restartable
  when done, and resumable when in progress.
- **R9 (event):** While Wizard mode is on, when the developer selects a control a
  task fits, the Wizard may show an **offer** beside it. An offer shall be
  dismissible, shall stay dismissed for that task on that control, and shall **never
  start a task by itself**.
- **R10 (state):** While Wizard mode is off, no offer shall be drawn and no step of
  the Wizard shall run.

### Running a task

- **R11 (ubiquitous):** Each step shall dim the IDE except the **lit target**, draw a
  **speech balloon** whose tail points at it, and carry the task's title, "step *n* of
  *m*", the step's text, and **Back**, **Next** and **Exit**.
- **R12 (state):** The lit target shall be **live**: pointer input over it shall reach
  the component, and keyboard input shall reach the component that has focus
  inside it. Pointer input **outside** it shall not reach the IDE beneath, so a stray
  click cannot wander off the task.
  *(This is the deliberate difference from the Walkthrough, whose R4 and R6 keep the
  tour read-only.)*
- **R13 (ubiquitous):** The lit target may be **inside a modal or a floating window**
  (the Model Providers Manager, the Agents Manager, the Column Editor, the menu
  editor). The dim shall cover everything else, other windows included, and shall not
  cover the window that holds the target.
- **R14 (ubiquitous):** Each step shall declare a **completion condition** that can be
  read from the IDE's own state — a field now holds a value, a provider was saved, a
  control is selected, a section was opened, a property was set. When it holds, the
  Wizard shall acknowledge it and **advance by itself**.
- **R15 (ubiquitous):** **Next** shall always be available, whether or not the
  condition holds. A step skipped with its condition unmet shall be listed in the
  closing summary as skipped.
- **R16 (state):** While a step's target is not on screen — a window closed, a pane
  collapsed, a tab switched — the Wizard shall **not point at nothing**. It shall
  either follow the target to where it now is, or pause the balloon in a corner with
  a **Continue** chip that says what to reopen, or, where reaching the target is one
  action the developer would take anyway, **offer to take it** and say so.
- **R17 (event):** When a task finishes, the Wizard shall show a **summary**: what was
  done, what was skipped, and where the result lives.
- **R18 (event):** **Exit**, `Esc`, or turning Wizard mode off shall leave the task
  *in progress at its current step*, and the catalogue shall offer to resume it.
  `Esc` shall end the wizard and shall **not** also close the window beneath on the
  same press.
- **R19 (ubiquitous):** The Wizard shall never run together with the Walkthrough. If
  the Walkthrough is running, a task cannot start; if the Walkthrough starts, a
  running task pauses and resumes when it ends.

### Doing the tedious part (optional per step)

- **R20 (optional):** Where a step is mechanical and safe, its balloon may also offer
  **Do this for me**. It shall do exactly what the developer would have done in the
  real editor — the same code path, the same undo entry — and shall say what it did.
- **R21 (constraint):** The Wizard shall never overwrite or delete anything the
  developer wrote, and shall never fill a field that already holds a value without
  asking. *(User code is sacred.)*
- **R22 (constraint):** The Wizard shall never enter, read or store an API key or
  other secret, never call a model or the network, never build, and never run the
  application. **Do this for me** shall not exist for any step whose action would.

### Memory

- **R23 (ubiquitous):** Each task's state — never started, in progress at step *n*,
  done — and each dismissed offer shall be **machine-level**, beside the switch
  (R4), and survive a restart.
- **R24 (ubiquitous):** The Wizard shall write nothing into a project folder for its
  own bookkeeping.

### Fit with the IDE

- **R25 (ubiquitous):** Every string the Wizard shows — toolbar tips, the menu item,
  the catalogue, every step's title and text, buttons, summaries, reasons — shall be a
  `Tr` field translated in all six languages (EN/ES/PT/JA/ZH/FR).
- **R26 (ubiquitous):** The Wizard shall follow the active IDE theme and stay legible
  on all 32 themes, light and dark.
- **R27 (constraint):** The Wizard shall **never resize any window** (`CONVENTIONS.md`).
  The balloon is sized from explicit numbers and placed by a pure function, never
  from the space it was handed.
- **R28 (constraint):** While Wizard mode is off the Wizard shall cost **one boolean
  read per frame** and nothing else: no anchor collection, no allocation.
- **R29 (ubiquitous):** The Wizard's logic — which step shows, whether its condition
  holds, where the balloon goes, what is dim — shall be **testable without the
  application object**, which cannot be built in a test. It shall be handed what the
  frame painted and a read-only snapshot of the IDE's state, and shall return what it
  wants done, as the Walkthrough does.
- **R30 (ubiquitous):** A task shall be **a declarative definition** — its steps,
  each with a target, a text key and a completion condition — held in one table.
  Adding a task shall not change the engine.
- **R31 (constraint):** This feature shall **not change the Walkthrough's behaviour**:
  every acceptance criterion of spec 059 keeps passing.

## 6. Acceptance criteria

- [ ] **AC1** — Both toolbars carry the toggle. Pressing either changes both; the
      state survives an IDE restart and is **off** on a fresh `ui.toml`.
- [ ] **AC2** — Engaged on both toolbars on all 32 themes, Aurora Pastel's pills
      included (asserted on the painted button, not on a predicted colour).
- [ ] **AC3** — The Help menu's item mirrors the state and changes it.
- [ ] **AC4** — With no project open the switch turns on, and a task that needs one
      is disabled with its reason and cannot start.
- [ ] **AC5** — The catalogue lists the eight tasks of §3 with the right state, and
      each can be started, resumed after Exit and restarted when done.
- [ ] **AC6** — For **every step of every task**, the lit region equals the rect of
      the component the step names, as the frame actually painted it, and the
      balloon's tail touches it. Asserted per step against painted rects.
- [ ] **AC7** — A click inside the lit region reaches the component; a click outside
      it does not reach the IDE; text typed into a lit text field arrives.
- [ ] **AC8** — A target inside a modal or floating window is lit while the dim covers
      every other window and not the one that holds it.
- [ ] **AC9** — A step whose condition becomes true advances by itself; **Next**
      advances when it does not; **Back** returns; a skipped step is in the summary.
- [ ] **AC10** — Closing the lit window mid-step leaves no balloon pointing at empty
      space: it follows, or pauses with a Continue chip that names what to reopen.
- [ ] **AC11** — **Exit**, `Esc` and the toggle all leave the task in progress at the
      same step and the catalogue resumes it; `Esc` does not also close the window
      beneath.
- [ ] **AC12** — While the Walkthrough runs, no task starts; starting the Walkthrough
      pauses a running task, which resumes after it.
- [ ] **AC13** — Every string exists in all six languages, the i18n completeness test
      passes, and a rendered pass over the catalogue and one balloon of each task shows
      **no word left in English** in the other five.
- [ ] **AC14** — Balloon text and its backdrop meet the contrast the IDE already
      requires, measured on the **painted** colours, on a light theme and a dark one.
- [ ] **AC15** — No window changes size during any task, measured over rendered frames
      in all six languages.
- [ ] **AC16** — With Wizard mode off, a frame does one boolean read of the Wizard's
      state (asserted by a counter, not by timing).
- [ ] **AC17** — A fixture task added in a test, as a definition alone, appears in the
      catalogue and runs, with no change to the engine.
- [ ] **AC18** — Completing each task through the Wizard yields **the same project
      files** as the same task done by hand, byte for byte where the editor is
      deterministic.
- [ ] **AC19** — **Do this for me**, where a step has it, produces the same change as
      the manual route, is undone by one undo, and refuses to overwrite a field that
      holds a value without asking.
- [ ] **AC20** — No API key appears in `ui.toml`, in any log, or in the Wizard's
      state, in a test that types a recognisable key into the real field.
- [ ] **AC21** — Every acceptance criterion of spec 059 still passes unchanged.

## 7. Constraints & steering check

- **i18n (6 languages):** **yes, and large.** Eight tasks of several steps, each with a
  title and a text, plus the catalogue, buttons, summaries, reasons and offers: on the
  order of a hundred `Tr` fields × six languages. No literal in a panel.
- **Generated-code contract:** none directly — the Wizard writes no COBOL. The editors
  it guides do, and they do it exactly as before (R20, AC18).
- **System KB:** **no** — no control, property, method or event is added or changed.
  `/plan` must confirm this rather than assume it; if any control doc changes,
  `chunked.data` is regenerated in the same change.
- **Docs:** **yes** — `docs/developers-guide-en.md` gains a section on Wizard mode and
  the task catalogue. Translation policy is GOLDEN RULE #8: the English canonical is
  updated, and the translations of a touched document are regenerated at the next
  minor, not patched.
- **Fix vs feature:** **feature** — a new IDE capability. `features` branch (a
  `features-<slug>` branch off `main` while another checkout holds `features`); `z` bump
  only, the minor is the operator's; announced on f=96 only if the operator asks.
- **Themes:** legible on all 32; hardcoded contrast where `ui.visuals()` cannot be
  trusted, as the Walkthrough and the chat balloons do.
- **A window may never resize itself:** R27, AC15.
- **Privacy:** R22, AC20 — no secrets, no network, no telemetry.
- **Tests:** the engine is pure (R29) so that it can be tested; the app object cannot be
  constructed in a test and the project does not drive the application's window.

## 8. Decisions already taken

From the operator's request (2026-10-17):

- **The Wizard is of the Tour type** — the dim, the lit target and the comic balloon of
  spec 059.
- **It is switched on and off by a toggle button** on the IDE toolbar **and** on the
  Form Designer (RAD) toolbar.
- **The first subjects** are models, agents, databound controls, DataGrids, TreeViews,
  SideMenus, menus and animations, and the list is open ("etc.").

## 9. Settled by the author, and easy to reverse

Design calls inside the scope the operator set, resolved here with the reasoning so a
different call is a one-line change rather than an argument.

- **One shared, machine-level switch.** Two buttons with two states would disagree the
  moment the developer used both. It is stored with `UiPrefs`, beside
  `walkthrough_shown`, for spec 059's reason: a way of working is a property of the
  developer, not of one project, and a colleague opening the same project must not
  inherit it.
- **The lit target is live.** This is the one place the Wizard must part from the
  Walkthrough. A task cannot be done on a component the developer is not allowed to
  touch, and spec 059 R4 and R6 exist precisely because a *tour* must not be
  half-dismissed by a stray click. The Wizard keeps the second half of that
  reasoning — the dim blocks everything *outside* the target — and drops the first.
- **Completion by observed state, with Next as the override.** A wizard that waits for
  an event that never comes traps the developer; one that only has a Next button is a
  slide deck. Observing the IDE's state is what makes it a wizard and Next is what
  keeps it from being a trap.
- **Default off.** Wizard mode is for the developer who asks for it. A machine that has
  never set the switch behaves exactly as the IDE did before this feature.
- **The Walkthrough is left alone.** It describes the IDE once; the Wizard teaches
  tasks, repeatedly, on demand. Merging them would change a tour whose shape the
  operator settled one decision at a time (spec 059 §8).
- **No secrets, ever.** The agent and provider tasks end at a field that takes a key.
  The Wizard lights the field and says what goes there; it does not touch what is typed.

## 10. Open questions

Each carries a recommendation, so `/clarify` can settle all seven in one answer.

- **Q1 — Name.** The IDE already has dialog "wizards". Calling this one *Wizard* in the
  interface will blur the two. **Recommend:** the toolbar tip and the Help item read
  **Task Wizard**; the internal word is *task*.
- **Q2 — Does it act, or only guide?** R20 makes **Do this for me** optional per step.
  **Recommend:** the engine and all eight tasks ship **guide-only**; **Do this for me**
  follows for the few mechanical steps (a default column set, a starter menu), where
  the manual route is tedious and the result is easy to undo.
- **Q3 — Where does the catalogue open?** **Recommend:** a popup under the toggle
  button, so the switch and its list are one place; the Help item opens the same popup.
- **Q4 — Offers (R9).** Are contextual offers in the first release, or only the
  catalogue? **Recommend:** both, since an offer is what makes the Wizard findable by
  the developer who has never heard of it; dismissible per task and per control.
- **Q5 — Phasing.** All eight tasks need anchors and completion conditions in eight
  different panels, which is the real work. **Recommend:** release in waves — the engine
  plus **models, agents and DataGrid** first, because they cross the most surfaces and
  prove the mechanism; the rest follow as definitions.
- **Q6 — A first-run nudge.** Should the Walkthrough's last balloon mention the switch?
  That changes a settled tour. **Recommend:** no; the Help menu item and the toolbar
  button are the discovery, and the Walkthrough is not amended.
- **Q7 — Per machine, or per project, for *done*.** **Recommend:** per machine (R23),
  like the switch: it records what the *developer* has learned.
