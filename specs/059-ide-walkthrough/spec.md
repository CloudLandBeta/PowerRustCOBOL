# Spec — IDE Walkthrough

- **Status:** draft → **ready for `/plan`** (every question settled; no implementation yet)
- **Folder:** specs/059-ide-walkthrough/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-09

## 1. Overview

A **Walkthrough**: a short, optional guided tour of the IDE that introduces its
six main components in order, each explained by a **comic-style speech balloon**
whose tail points at the component being described, while the rest of the IDE
dims behind it. It runs once — the first time a developer opens a project on
this machine — and then switches itself off; a checkbox in the **Help menu**
turns it back on, and unchecking it starts the tour immediately.

It exists because the IDE's own shape is the first thing a COBOL developer has
to learn and the last thing the product explains. The Developer's Guide covers
every component in depth, but nothing in the IDE says *what the seven nodes of
the project tree are for* at the moment a developer first sees them.

## 2. Goals / Non-goals

**Goals**

- Six steps, in a fixed order, each naming one component and saying what it does.
- The component under discussion is unmistakable: everything else dims, it stays
  lit, and the balloon points at it.
- It never has to be sat through: Skip leaves at any step.
- It runs once per developer, not once per project, and it is trivial to replay.

**Non-goals**

- **Teaching the components.** Each balloon is one or two sentences; depth stays
  in the Developer's Guide.
- **A tutorial that makes the developer do things.** The tour is read, not
  performed (the operator chose the spotlight over a click-to-advance
  tutorial — see §8).
- **A second walkthrough per feature.** This describes the IDE's main components
  once; it is not a framework for onboarding every future panel.
- **Replacing the AI-setup invitation** (`hide_ai_setup_prompt`), which is a
  different prompt with its own dismissal.

## 3. The six steps

The order and the substance are the operator's (2026-09-09). The English text
below is the canonical source for translation; wording may be tightened during
`/plan` but the subject of each step may not change.

| # | Component | What the balloon says |
|---|---|---|
| 1 | **Project settings** | Configure IDE / form themes, AI model and agent configuration, form effects and more. |
| 2 | **Forms** | The tree-organised application forms, where forms are created and modified. |
| 3 | **Indexed Files** | The ISAM indexed-file editor and visualiser. |
| 4 | **Assets** | Images, documents and whatever else your application requires. |
| 5 | **Knowledge Base** | Your project's KB — specifications, legal information, application-domain knowledge, and anything the agents need in order to generate code well. |
| 6 | **The Output pane** | Where the IDE answers you: build and compile messages, program output, diagnostics and the results of what you run. |

Every one of these already exists in the IDE and is a real anchor:
`Category::Forms`, `Category::IndexedFiles`, `Category::Assets` and
`Category::Documentation` (which is the **Knowledge Base** node —
`panels/project.rs` sets `is_knowledge_base = cat == Category::Documentation`
and its label is `tr.cat_documentation`, "Knowledge Base"), the project tree's
**root node** for project settings, and `panels/output.rs` for the output pane.

## 4. User stories

- As a COBOL developer opening PowerRustCOBOL for the first time, I want to be
  shown what the parts of the window are for, so that I can start without
  reading a manual first.
- As a developer who knows the IDE, I want the tour to disappear after one
  showing and never interrupt me again.
- As a developer who dismissed it too quickly, I want to bring it back from the
  Help menu and have it start at once.
- As a developer who works in six languages, I want the balloons in mine.

## 5. Requirements (EARS)

### Running the tour

- **R1 (ubiquitous):** The Walkthrough shall present the six components of §3 in
  that order, one step at a time.
- **R2 (ubiquitous):** Each step shall draw a **speech balloon** — a rounded
  body with a **tail pointing at the component** it describes — carrying that
  step's text.
- **R3 (state):** While the Walkthrough is running, the IDE shall be **dimmed
  except for the component under discussion**, which stays at full brightness.
- **R4 (state):** While the Walkthrough is running, pointer and keyboard input
  shall go to the Walkthrough and not to the IDE beneath it, so that the tour
  cannot be half-dismissed by a stray click.
- **R5 (ubiquitous):** Each step shall offer **Next**, **Back** and **Skip**;
  the last step's Next ends the tour. `Esc` shall end it like Skip.
- **R6 (constraint):** The Walkthrough shall **not** require the developer to
  operate the component being described in order to advance.

### Starting and stopping

- **R7 (event):** When a project is opened and the Walkthrough has never been
  shown on this machine, the Walkthrough shall start.
- **R8 (event):** When the Walkthrough ends — by finishing, by Skip or by
  `Esc` — it shall set its own "never show this again" flag, so it does not
  start on the next project.
- **R9 (constraint):** The flag shall be **machine-level**, stored with the
  developer's other IDE preferences rather than in `cobolt.toml`: a tour of the
  IDE is learned once, not once per project, and a colleague opening the same
  project must not inherit someone else's answer.
- **R10 (ubiquitous):** The **Help menu** shall carry the flag as a checkable
  item, showing its current state.
- **R11 (event):** When the developer **unchecks** that item, the Walkthrough
  shall start immediately.
- **R12 (state):** While no project is open, the Walkthrough shall not start —
  five of its six anchors are project-tree nodes and do not exist without one.
  The Help-menu item stays available, and unchecking it with no project open
  shall report that a project is needed rather than starting a broken tour.

### Fit with the IDE

- **R13 (ubiquitous):** Every string the Walkthrough shows — balloon text,
  button labels, the Help-menu item — shall be a `Tr` field translated in all
  six languages (EN/ES/PT/JA/ZH/FR).
- **R14 (ubiquitous):** The Walkthrough shall follow the active IDE theme, and
  shall stay legible on every one of the 32 themes, light and dark.
- **R15 (constraint):** The Walkthrough shall **never resize any window**
  (`CONVENTIONS.md`): it draws over the IDE as it is.
- **R16 (constraint):** A step whose component is not on screen — a collapsed
  or hidden pane — shall not point at nothing: it shall either reveal the
  component or be skipped, and never leave a balloon with a tail into empty
  space.

## 6. Acceptance criteria

- [ ] **AC1** — With the flag unset and a project open, the Walkthrough starts
      by itself and shows step 1 of 6.
- [ ] **AC2** — Each of the six steps highlights the correct component: the
      lit region matches that component's rect on screen, and the balloon's tail
      touches it. Asserted per step against the rects the frame actually
      painted, not against expected values.
- [ ] **AC3** — Next advances, Back returns, and Skip and `Esc` both end it,
      from any step.
- [ ] **AC4** — Ending it by any of the three routes sets the flag, and a
      second project opened afterwards does not start the tour.
- [ ] **AC5** — Unchecking the Help-menu item starts the tour on the same frame
      it is unchecked, with a project open; with none open it reports that and
      starts nothing.
- [ ] **AC6** — The flag survives an IDE restart (it is on disk with the other
      machine-level preferences).
- [ ] **AC7** — Every string appears in all six languages, and the i18n
      completeness test passes.
- [ ] **AC8** — Balloon text and its backdrop meet the contrast the IDE already
      requires, measured on the **painted** colours (never on a predicted
      surface colour), on a light theme and a dark one.
- [ ] **AC9** — No window changes size at any point in the tour.

## 7. Constraints & steering check

- **i18n (6 languages):** yes — six balloon texts, three button labels, one
  Help-menu item, one "open a project first" message. No hard-coded literals.
- **Generated-code contract:** none — the Walkthrough touches no form and emits
  no COBOL.
- **System KB:** **no** — this adds no control, property, method or event.
  (`/plan` must confirm this rather than assume it; if any control doc changes,
  `chunked.data` is regenerated in the same change.)
- **Docs:** yes — `docs/developers-guide-en.md` gains a short Walkthrough
  section under the IDE tour. Translations untouched.
- **Fix vs feature:** **feature** — a new IDE capability beyond existing scope.
  `features` branch; `z` bump; announced on f=96 only if the operator asks.
- **Themes:** it must read on all 32 IDE themes; chat-style hardcoded contrast
  is the established answer where `ui.visuals()` cannot be trusted.

## 8. Decisions already taken

- **The flag is machine-level, not per project** (operator, 2026-09-09),
  overriding the original request's "in Project Settings". It therefore belongs
  with `UiPrefs` (`<data_dir>/cobolt/ui.toml`), beside `rust_check_done` — the
  existing "this first-run question has been settled" flag — rather than with
  `IdeSettings` in `cobolt.toml`.
- **The switch lives in the Help menu** (operator, 2026-09-09), not in the
  Project Settings form.
- **Spotlight style** (operator, 2026-09-09): dim the IDE, highlight the target,
  block input beneath. Not a non-blocking balloon, and not a
  click-the-component-to-advance tutorial.
- **The Output pane's balloon** (operator, 2026-09-10): *"Where the IDE answers
  you: build and compile messages, program output, diagnostics and the results
  of what you run."* — the last of the six texts, and the one the operator's
  original list left unwritten.

## 9. Settled by the author, and easy to reverse

Three questions the operator did not need to answer, resolved here with the
reasoning so a different call is a one-line change rather than an argument.
Every one of them is a design detail inside the scope the operator already set;
none changes what is built.

- **Skip counts as shown** (R8 as written). Any exit — finishing, Skip, `Esc` —
  sets the flag. A tour that comes back after being skipped is a nag, and the
  Help menu makes replaying it trivial, so the cost of being wrong is one menu
  click. The alternative (only finishing counts) would show it again to the very
  developer who has just said no.

- **Step 1 points at the project tree's root node**, and the Walkthrough does
  **not** open the settings form to make a better picture. The tour describes the
  IDE; it must not rearrange the IDE while describing it, or the developer ends
  the tour somewhere they did not put themselves. The balloon says what the root
  node is *for*, which is what a tour is.

- **One showing per machine** (R7 as written). A second machine shows it again,
  which is right — it is a new place to learn. Wiped preferences show it again,
  which is also right. Nothing re-arms it on upgrade: a developer who knows the
  IDE does not need re-teaching because the version changed.

## 10. Open questions

*(None. The spec is ready for `/plan`.)*
