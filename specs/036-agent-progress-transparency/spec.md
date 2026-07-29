<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Agent Progress Transparency (Live Action Status)

- **Status:** draft
- **Folder:** specs/036-agent-progress-transparency/
- **Author:** Emerson   **Date:** 2026-07-28

## 1. Overview

While Grace or a specialist agent is reasoning, producing results, or reviewing
and applying corrections, a run can take minutes — and today the conversation
history pane shows only a spinner and a static "Coordinating specialists…"
label until the work finishes. Nothing proves the agents are doing anything,
which erodes trust during long runs. This feature makes every agent
continuously surface **what it is doing** in the conversation history pane as
short, human-readable action status lines that update in near-real-time, so
the user can always tell at a glance that the system is active and what step
it is on. Status lines name **actions only** — the data an action produced or
consumed stays in the IDE log pane, where it already lives.

## 2. Goals / Non-goals

- **Goals:**
  - Continuous, glanceable progress signalling in the conversation history
    pane for every agent that runs work on the user's behalf: the
    orchestrator (**Grace**) and all **specialist** agents it delegates to.
  - Each agent reports its own actions, attributed by agent, so the user can
    tell who is doing what when several agents are active.
  - A persisted, collapsed history of completed action lines, so the sequence
    of steps taken during the run can be reviewed.
  - Verbose mode (the existing `verbose_log` setting) yields a richer,
    per-step action stream — more granularity, never payloads.
- **Non-goals:**
  - Rendering or formatting of retrieved context/tool output itself — that
    remains the IDE log pane's job, unchanged.
  - A new verbosity toggle or setting (reuse `verbose_log`).
  - The internal mechanism agents use to detect/emit action boundaries
    (implementation detail for `/plan`).
  - Progress display for non-agent work (builds, indexing, etc.).

## 3. User stories

- As a developer running a long Grace workflow, I want to see a live line
  naming the current step (e.g. "Retrieving context for control(s)"), so that
  I know the system is working and not stuck.
- As a developer reviewing a finished (or in-flight) run, I want the completed
  action lines kept as a collapsed history in the conversation pane, so that
  I can see the sequence of steps that were taken.
- As a developer with several agents active, I want each status line
  attributed to the agent performing it, so that I can tell Grace's actions
  from a specialist's.
- As a developer with verbose mode on, I want a more detailed per-step action
  stream, so that I can follow the run closely without opening the log pane.
- As a developer who needs the underlying data, I want the retrieved context
  and tool payloads to stay in the IDE log pane only, so that the
  conversation pane stays lightweight and readable.

## 4. Requirements (EARS)

- **R1 (state):** While an agent (Grace or a specialist) is executing a run,
  the conversation history pane shall display a status line naming the
  agent's current action.
- **R2 (event):** When an agent starts a new meaningful action (e.g.
  retrieving context, analyzing, drafting, reviewing, applying corrections),
  the system shall append a new status line for it; prior lines are not
  replaced.
- **R3 (ubiquitous):** Completed action lines shall persist in the
  conversation history pane as a collapsed history for the run, expandable by
  the user.
- **R4 (constraint):** Status lines shall describe the **action only**; the
  system shall never place retrieved context, tool payloads, intermediate
  reasoning, or returned results in the conversation history pane — in any
  mode. Full data remains available in the IDE log pane.
- **R5 (optional):** Where verbose mode (`verbose_log`) is enabled, agents
  shall emit a more detailed per-step action stream in the conversation
  history pane; verbose output is still subject to R4.
- **R6 (ubiquitous):** Status line updates shall be rate-limited to at most
  one displayed line per second; actions occurring faster shall be coalesced
  without losing any action from the persisted history.
- **R7 (state):** While more than one agent is active, each status line shall
  be attributed to the agent performing it.
- **R8 (ubiquitous):** Status lines shall be concise, present-tense, plain
  language for a human reader (e.g. "Retrieving context for control(s)"),
  never internal function names or identifiers.
- **R9 (constraint):** The canonical action-line vocabulary rendered by the
  IDE shall be localized `Tr` strings in all six languages; dynamic
  fragments (control names, file names, agent names) are interpolated
  verbatim.
- **R10 (ubiquitous):** Every chat surface that hosts a Grace/specialist run
  (the project Grace chat and any other conversation history pane that runs
  agents) shall show the same status behaviour.
- **R11 (ubiquitous):** The action-line history shall be persisted with the
  run's workflow record / chat history, so a past run's steps can be
  reviewed in a later session.

## 5. Acceptance criteria

- [ ] AC1 — During a multi-step Grace run, the conversation pane shows a
  changing action line while work is in flight; at no point is the only
  signal a spinner (R1, R2).
- [ ] AC2 — After (and during) a run, expanding the collapsed history shows
  the ordered sequence of action lines for that run, including coalesced
  fast actions (R3, R6).
- [ ] AC3 — With default (non-verbose) settings, no retrieved document text,
  tool payload, or model reasoning appears in the conversation pane; the
  same data is present in the IDE log pane (R4).
- [ ] AC4 — With `verbose_log` on, the pane shows a per-step action stream
  with more granularity than default mode, and still no payloads (R5, R4).
- [ ] AC5 — Emitting >1 action per second updates the visible line at most
  once per second, and the persisted history still contains every action
  (R6).
- [ ] AC6 — In a run where Grace delegates to a specialist, lines from Grace
  and from the specialist are visibly attributed to their agent (R7).
- [ ] AC7 — Action lines read as plain human phrases; a review of the emitted
  vocabulary finds no internal function names (R8).
- [ ] AC8 — Switching the IDE language changes the canonical action-line
  vocabulary; all six languages have translations (R9).
- [ ] AC9 — Reopening a project after a finished run, the run's collapsed
  action history is still reviewable from the saved record/chat history
  (R11).

## 6. Constraints & steering check

- **i18n (6 languages):** yes — every canonical action-line string and any
  new UI chrome (e.g. the collapsed-history header) are `Tr` fields in
  `i18n.rs` translated in EN/ES/PT/JA/ZH/FR (R9).
- **Generated-code / regenerate contract:** no impact — IDE UI + agent host
  only; no `.cbl` generation touched.
- **Docs (English guide):** yes — update `docs/developers-guide-en.md`
  (Grace/AI assistant section) to describe default vs verbose status lines
  and the action-vs-context boundary. Translations untouched (user-
  maintained).
- **Chat UI colors:** status lines live in the chat pane — follow the
  hardcoded-contrast rule (balloon palette), never `ui.visuals()`-derived
  colors.
- **Fix vs feature:** **feature** — bump the **minor** version in
  `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` entry; branch
  `feat/agent-progress-transparency`.

## 7. Resolved decisions (from the draft)

- **Line persistence:** completed action lines persist as a collapsed
  history in default mode; new actions append rather than replace.
- **Verbose mode:** reuse the existing `verbose_log` setting; no new toggle.
- **Throttle:** at most 1 displayed line per second; faster actions coalesce,
  none are dropped from the persisted history.
- **Persistence scope (resolved 2026-07-28):** the action history is saved
  with the workflow record / chat history and reviewable in later sessions
  (R11).
- **i18n scope (resolved 2026-07-28):** only the canonical action vocabulary
  rendered by the IDE is `Tr`-localized; dynamic/verbose fragments emitted by
  agents appear as-is (R9).

## 8. Open questions

None — all resolved above.
