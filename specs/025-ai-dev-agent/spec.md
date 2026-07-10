<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — AI Development Agent (dev-time form-building assistant)

- **Status:** draft → approved
- **Folder:** specs/025-ai-dev-agent/
- **Author:** PowerRustCOBOL / dev-agent   **Date:** 2026-07-08

## 1. Overview

PowerRustCOBOL already ships a dev-time AI assistant (`cobolt-ide::llm`,
`LlmConfig` — a local or cloud chat-completions endpoint) that today only
**rewrites the whole COBOL source** shown in the editor. This feature promotes
that assistant into a **development agent** that acts on the *form and project*,
not just editor text: on an explicit developer request it proposes a structured
change-set — **deploy new controls, edit any property of any control, generate
COBOL event-handler code, and create common procedures** — which the developer
**previews and approves** before anything is applied. It reuses the existing IDE
`LlmConfig` (Settings → AI assistant) and is surfaced through the existing
**editor prompt bar**. The agent never acts on its own; it only responds to a
direct request.

## 2. Goals / Non-goals

**Goals**
- Turn natural-language requests into a **structured, reviewable change-set** over
  the current form + project.
- Cover four capabilities: (a) deploy new controls, (b) edit **any** property of
  **any** existing control, (c) generate COBOL event-handler code for a control
  event, (d) create common (shared) procedures.
- **Preview-then-approve**: nothing mutates until the developer approves; applied
  changes are a **single undoable action**.
- Reuse the existing `LlmConfig` (local or cloud) and the editor prompt bar — no
  new connection settings, minimal new surface.

**Non-goals**
- **Not** the runtime `AgentObject` control (that configures an LLM baked into the
  *generated app*; this feature is dev-time only and stays separate).
- **Not** autonomous: no background polling, scheduled runs, or auto-apply.
- **Not** hand-editing the throwaway generated `.cbl` wrapper (build artifact).
- **Not** whole-app generation or multi-form refactors in one shot — scope is the
  current form and its event/procedure code.
- Control **deletion/rename** by the agent is out of scope for this spec
  (deploy + edit-property + generate-code only). See Open Questions.

## 3. User stories

- As a COBOL developer, I want to type "add a Save and a Cancel button" and get a
  previewed change-set I can approve, so I place common controls without hunting
  the toolbox.
- As a developer, I want to say "make TotalLabel bold, right-aligned, and green"
  and have the agent set those properties on that exact control, so I skip the
  property grid.
- As a developer, I want "generate an onClick handler for SaveButton that writes
  the record and refreshes the grid", so I get correct COBOL scaffolding bound to
  the event.
- As a developer, I want "create a VALIDATE-INPUT procedure", so shared logic is
  added once and callable from handlers.
- As a cautious developer, I want to **review every proposed change and Undo it in
  one step**, so the agent can never silently alter my form.

## 4. Requirements (EARS)

- **R1 (state):** While the IDE `LlmConfig` is configured (`is_configured()` —
  endpoint **and** model set), the dev agent shall be available from the editor
  prompt bar; while it is not configured, the agent shall be unavailable.
- **R2 (event):** When the developer submits a request from the prompt bar, the
  system shall send that request to the configured LLM together with **context**:
  the current form's control inventory (id, type, current properties) plus the
  supported control-type / property-key / event-name schema.
- **R3 (event):** When the LLM replies, the system shall parse the reply into a
  **structured change-set** — an ordered list of operations from a fixed
  vocabulary: `deploy_control`, `set_property`, `generate_event_handler`,
  `create_procedure`.
- **R4 (ubiquitous):** The system shall support in a change-set: (a) deploying a
  new control of **any** supported `ControlType`; (b) setting **any** property key
  to a value on **any** existing control identified by id; (c) generating COBOL
  event-handler code bound to a specific control **event**; (d) creating a named
  common procedure.
- **R5 (event):** When a change-set is produced, the system shall present a
  **preview** listing every operation — new controls (type + placement), property
  changes shown as *before → after*, and generated code — and shall **not** mutate
  the form, project, or code until the developer explicitly approves.
- **R6 (event):** When the developer **approves** a previewed change-set, the
  system shall apply all operations as a **single undoable action** (mapping to
  the existing `Cmd` history), so one Undo reverts the entire change-set.
- **R7 (event):** When the developer **rejects/cancels** a preview, the system
  shall discard the change-set and leave the form, project, and code unchanged.
- **R8 (constraint):** The system shall **not** initiate agent actions
  autonomously — it acts **only** in direct response to a developer request; there
  shall be no background polling, scheduled runs, or auto-apply.
- **R9 (constraint):** The system shall **not** apply an operation that references
  an unknown control id, an unsupported control type, or an invalid property key /
  value; such operations shall be shown in the preview as **errors** and blocked
  from being applied.
- **R10 (constraint):** Generated COBOL (event handlers, procedures) shall keep all
  identifiers and source text **English** and integrate through the existing
  event-binding + generated-code contract (banner via `write_header`,
  regenerate-on-Build/Run/Debug/Check); the agent shall never hand-edit the
  generated `.cbl` wrapper.
- **R11 (constraint):** Every new user-facing string (prompt hint, preview labels,
  Approve/Reject, status, errors) shall be a `Tr` field translated in **all six**
  languages (EN/ES/PT/JA/ZH/FR).
- **R12 (state):** While an agent request is in flight, the IDE shall stay
  responsive (async, like the current assistant) and allow **cancel**; a failed or
  malformed reply shall be surfaced to the developer and apply **nothing**.
- **R13 (optional):** Where a deployed control omits geometry, the system shall
  place it with sensible, non-overlapping defaults so the developer can rearrange
  it.
- **R14 (ubiquitous):** The system shall send the **effective** agent system prompt
  — the project's customised prompt when set, otherwise the built-in
  `AGENT_SYSTEM_PROMPT` default — as the system message on **every** request; the
  prompt is applied at send time and is **never** written to the conversation
  memory.
- **R17 (event):** The system shall let the developer edit the agent system prompt
  from **project settings**: a button opens a **modal editor window** (multiline
  editor) with **Save**, **Reset to default**, and **Cancel**. Save writes the
  prompt to the project's `agentic_ai/` folder (per R18); it becomes the effective
  prompt used by R14. Reset restores the built-in `AGENT_SYSTEM_PROMPT` default; a
  missing/empty prompt file also falls back to that default.
- **R18 (ubiquitous):** All agent-AI resources shall live in a per-project
  **`agentic_ai/`** folder: the editable **system prompt** file (this feature),
  with the folder reserved to also hold **agent definitions** and **skills** as the
  agentic capability grows. The effective prompt (R14) is read from it.
- **R19 (event):** When a project is **created or opened**, the system shall ensure
  the `agentic_ai/` scaffold exists: if the folder or any default markdown file is
  missing, it shall create the folder and (re)seed only the **missing** defaults —
  `system-prompt.md` (`AGENT_SYSTEM_PROMPT`) and `skills/rustcobol-extensions.md`
  (how RustCOBOL extends COBOL-85). Existing files are **never overwritten** (a
  developer's edited prompt/skill is preserved), so opening an older project brings
  it up to date without touching customisations.
- **R21 (ubiquitous):** The system shall include the applicable skill files from
  `agentic_ai/skills/` in the agent's context on **every** request — **always**
  including `rustcobol-extensions` — so the agent is oriented on RustCOBOL's
  extensions and never assumes plain COBOL-85 for GUI/property access. Skills are
  applied at send time and are **not** stored in the conversation memory.
- **R20 (state):** The project tree shall show an **"Agentic AI"** category at the
  **top level** (a peer of Forms, Indexed Files, …) that lists the files under
  `agentic_ai/`. Opening a file from that node shall open it in the IDE editor, and
  **any** file under `agentic_ai/` shall be editable and saveable from the IDE.
- **R15 (state):** The system shall keep a **local conversation memory** for the
  dev agent — an **indexed file keyed to the current form/project** — recording the
  developer's requests and the agent's replies, and shall include that memory on
  each request so the agent stays aware of the ongoing conversation. The memory
  persists across IDE restarts.
- **R16 (constraint):** The conversation memory shall contain **only conversational
  turns** (developer request text + agent reply). It shall **not** contain the
  system prompt (R14), the injected **skills** (R21), nor the per-request CONTEXT
  snapshot (R2) — all applied fresh per request — so stored history never carries a
  stale form snapshot or bloats with static reference text.

## 5. Acceptance criteria

- [ ] **AC1** — With `LlmConfig` unset the dev-agent prompt is unavailable; setting
      endpoint + model enables it. (R1)
- [ ] **AC2** — "Add a Save button" → preview shows one `deploy_control` (Button);
      Approve adds exactly one Button; a single Undo removes it. (R4a, R5, R6)
- [ ] **AC3** — "Make Label1 bold and green" → preview shows `set_property` for
      `Label1` (`Bold`, `ForegroundColor`) as before→after; Approve applies both;
      one Undo reverts both. (R4b, R6)
- [ ] **AC4** — A request naming a non-existent control id shows that operation as
      an error in the preview and applies nothing for it. (R9)
- [ ] **AC5** — "Generate an onClick handler for Button1 that clears the form" →
      preview shows generated COBOL bound to `Button1.onClick`; Approve inserts it
      via the event-code path; identifiers are English. (R4c, R10)
- [ ] **AC6** — "Create a procedure VALIDATE-INPUT" → preview shows a
      `create_procedure`; Approve adds it. (R4d)
- [ ] **AC7** — No agent call occurs without a developer-submitted prompt; verified
      by review — no timer/poll/auto path. (R8)
- [ ] **AC8** — Rejecting a preview leaves the form file and event/procedure code
      byte-identical. (R7)
- [ ] **AC9** — All new UI strings exist in all six i18n tables. (R11)
- [ ] **AC10** — A malformed / non-conforming LLM reply surfaces an error and
      applies nothing. (R12)
- [ ] **AC11** — An in-flight request can be cancelled and the IDE stays
      responsive. (R12)
- [ ] **AC12** — Every request carries `AGENT_SYSTEM_PROMPT` as the system message;
      inspecting the stored memory file shows the prompt is **absent**. (R14, R16)
- [ ] **AC13** — Multi-turn continuity: "add a Save button" then "make it green"
      resolves "it" from stored history; the memory reloads after an IDE restart and
      the next request still includes prior turns. (R15)
- [ ] **AC14** — The stored memory contains only developer/agent turns — no CONTEXT
      snapshot and no system prompt. (R16)
- [ ] **AC15** — Project settings has a button that opens a modal editor showing the
      current agent prompt (default text when unset); editing + Save writes it to the
      project's `agentic_ai/` folder; the next request's system message reflects the
      edit; Reset to default restores `AGENT_SYSTEM_PROMPT`; Cancel discards edits.
      (R17, R14, R18)
- [ ] **AC16** — Opening an **older project with no `agentic_ai/` folder** creates
      the folder and seeds `system-prompt.md` + `skills/rustcobol-extensions.md`; a
      project already having these opens with the files **unchanged**. (R19)
- [ ] **AC17** — Creating a new project produces an `agentic_ai/` folder containing
      the default system-prompt file **and** `skills/rustcobol-extensions`. (R19)
- [ ] **AC20** — If only one default is missing (e.g. the skill file was deleted),
      opening the project re-seeds **only** that file and leaves an edited
      `system-prompt.md` untouched. (R19)
- [ ] **AC19** — Every request's payload includes the `rustcobol-extensions` skill
      text; generated handler code omits `IDENTIFICATION`/`PROGRAM-ID`/`GOBACK` and
      uses `::` for property access; the skill is absent from stored memory.
      (R21, R16)
- [ ] **AC18** — The project tree shows an "Agentic AI" node at the same level as
      Forms/Indexed Files; it lists the `agentic_ai/` files; double-clicking one opens
      it in the IDE editor; editing and saving writes the file back. (R20)

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — new `Tr` fields for the prompt hint, preview panel
  (operation labels, before→after, generated-code header), Approve/Reject, in-flight
  and error status. All six languages required (tech.md hard constraint).
- **Generated-code / regenerate contract:** Impacted. Agent-generated **event
  handler bodies and common procedures are developer source** woven by
  `cobolt-codegen`, not edits to the throwaway `.cbl`; they must keep the banner /
  regenerate-on-action contract and English identifiers. The agent must not touch
  generated build artifacts.
- **Docs (English guide):** Yes — `docs/developers-guide-en.md` gains an "AI
  development agent" section (how to configure, how to prompt, preview/approve,
  undo). Translations are user-maintained (do not edit).
- **Fix vs feature:** **Feature** (new capability) per tech.md → minor version bump
  + CHANGELOG entry. (Operator's pre-prod convention may still version it as `z`;
  confirm at commit time.)
- **Separation:** Reuses `LlmConfig`; does not alter the runtime `AgentObject`.

## 7. Open questions

- **Q1 (wire format):** Exact structured format the LLM must return (JSON schema
  vs. provider function-calling vs. a tagged fenced block) so the change-set is
  robustly parseable and validated. → resolve in `/plan`.
- **Q2 (code storage):** Where event-handler bodies and common procedures live in
  the project model, and how `cobolt-codegen` weaves them — needed to define the
  `generate_event_handler` / `create_procedure` apply paths. → resolve in `/plan`.
- **Q3 (scope):** Should the agent later be allowed to **delete/rename** controls,
  or stay strictly deploy + edit-property + generate-code? (Out of scope here.)
- **Q4 (context budget):** Send the full form model or a compacted schema to fit
  the model's token limit? Strategy for large forms. → `/plan`.
- **Q5 (approve granularity):** Whole change-set approve (chosen) vs optional
  per-operation toggles in the preview — is per-op opt-out wanted?
- **Q6 (agentic_ai layout):** This feature places the **prompt** file in
  `agentic_ai/`. The on-disk **format/layout for agent definitions and skills**
  (subfolders, file types, how the IDE discovers/loads them) is reserved but **not
  specified here** — it needs its own spec once the multi-agent / skills capability
  is defined. Confirm that deferral.
