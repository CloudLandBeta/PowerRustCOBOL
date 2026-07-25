<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 030 — Runtime tool-execution layer for Grace's specialist agents

- **Status:** draft → approved
- **Folder:** specs/030-agent-tool-execution/
- **Author:** Claude (agent), for the operator   **Date:** 2026-07-18
- **Depends on:** spec 029 (Grace orchestrator — Phases A/B/C done), spec 028
  (agent database), spec 025 (AgentChangeSet preview/apply path).
- **Realises:** spec 029 **Phase C-next** (the remaining increment).

## 1. Overview

Grace's specialist agents today have definitions, prompts, per-agent models,
companion review gates, and end-to-end orchestration — but they **cannot act**.
Every `TaskResult` is text/evidence produced by an LLM completion; nothing the
agent "does" touches the form, the repository, or any external tool. This spec
adds the **runtime tool-execution layer**: the bridge that turns a specialist's
declared tool calls into real, evidenced actions during a Grace workflow, under
the tool/MCP governance already mandated by spec 029 R4.

Three concrete capabilities land:

1. **egui-MCP execution** — specialists can *observe* the live form/UI through
   the in-process egui inspection plugin (tree census, widget rects, screenshot)
   and capture what they see as evidence.
2. **Git execution for the Version Control Agent** — a bounded git executor that
   runs git **inside the user's currently-open project repository** (each
   PowerRustCOBOL project is its own repo), never the PowerRustCOBOL IDE/source
   repo.
3. **Applying approved form design** — an Approved form-design task's output is
   written back to the form through the existing, reviewable
   `AgentChangeSet` → preview → apply path, as one undoable action.

## 2. Goals / Non-goals

**Goals**

- Give specialist agents a single, governed way to execute declared tools inside
  a Grace workflow, with every action captured as tool evidence in the workflow
  record.
- Make form design *actually change the form*, but only via the proven,
  undoable, companion-reviewable change-set path.
- Let the Version Control Agent run real git against the **open user project's**
  repo, with a safety boundary that matches the operator's expectations
  (local free; network/history-rewrite gated).
- Preserve spec 029's governance: only declared tools run; fabricated ops or ids
  are critical defects that fail the task; nothing completes without execution +
  evidence.

**Non-goals**

- No live-UI *mutation* of the design via synthetic events — form design changes
  flow through the change-set path only (egui-MCP stays observe/verify).
- No git operations on the PowerRustCOBOL IDE/source repository, or on any path
  outside the currently-open project. This layer is not a general shell.
- No new external MCP transport work: in-IDE agents drive the existing
  in-process inspection plugin; the loopback `egui-mcp` bridge is unchanged.
- No autonomous network git (push/fetch-write/pull) without explicit operator
  confirmation.
- No changes to Grace's planning, review gates, or correction loops (029 C is
  done); this spec only adds the *execution* the plan already assumes.

## 3. User stories

- As a developer, I ask Grace to redesign a form; the Form Designer specialist's
  approved output is applied to my form as a single undoable change I can inspect
  and reverse, not just described in text.
- As a developer, a specialist can look at the live form (its widget tree /
  a screenshot) to verify its own work before claiming the task done.
- As a developer, I ask Grace to commit my project; the Version Control Agent
  runs real git in **my project's** repo, shows me the exact commands and their
  output, and stops for my explicit OK before anything leaves my machine
  (push) or rewrites history.
- As the operator, I trust that an agent can never invent a git result, touch a
  repo other than the open project, or apply a form edit that skipped review.

## 4. Requirements (EARS)

**Tool-execution bridge & governance**

- **R1 (ubiquitous):** The system shall provide a runtime tool-execution bridge
  that, during a Grace workflow, parses a specialist's declared tool calls,
  executes each against the corresponding tool backend, and returns the
  structured result to the agent as `TaskResult` evidence.
- **R2 (constraint):** The system shall execute only tools **declared** for the
  invoking agent (its `mcp.json` / `tools`). An undeclared/unknown tool, a
  malformed call, or a reference to a fabricated control/id/resource shall be
  treated as a **critical defect** that fails the task — never silently
  completed. (spec 029 R4.)
- **R3 (constraint):** The system shall not report any tool result that was not
  produced by a real execution; agents shall not "claim done" with simulated
  output. "done" without execution evidence is rejected (spec 029 delegation
  contract).
- **R11 (observability):** For every executed tool call the system shall record —
  in the workflow record (`agentic_ai/Grace/runs/<id>.json`) and the AI activity
  log — the tool name, inputs, a result/exit summary, and a timestamp. (spec 029
  observability.)

**egui-MCP execution (observe / verify)**

- **R4 (event):** When a specialist calls an egui inspection tool, the system
  shall execute it against the in-process inspection plugin (`ctx.with_plugin`)
  and return the observed result (e.g. widget tree census, widget rects,
  screenshot reference) as evidence.
- **R5 (constraint):** egui-MCP tools exposed to specialists shall be
  **observe/verify only**; the system shall not mutate form design through
  synthetic UI events. (Design mutation is R6/R7.)

**Form design via the reviewable change-set path**

- **R6 (ubiquitous):** The system shall apply specialist form-design output to a
  form **only** through the existing `AgentChangeSet` → `AgentPreview::build` →
  `validate` → apply path (one undoable action). No other write path to the form
  model is permitted for agent design output.
- **R7 (event):** When a Grace form-design task reaches **Approved** (passed its
  companion review), the system shall build a change-set from the approved
  output and apply it to the target form as one undoable action.
- **R8 (constraint):** Where an approved change-set fails validation (unknown
  control, invalid property, etc.), the system shall **not** partially apply it;
  the task shall be marked Failed/Blocked with the validation error as evidence,
  and the form left unchanged.

**Git execution — scoped to the user's open project repository**

- **R9 (ubiquitous, scope):** The Version Control Agent's git executor shall run
  git **only** within the currently-open user project's repository
  (`project_dir`). It shall never operate on the PowerRustCOBOL IDE/source
  repository, nor on any path outside the open project. The subprocess working
  directory is bound to the project root.
- **R10 (state):** While no project is open, or the open project is not a git
  repository, git tool calls shall fail cleanly with a clear message; the
  executor shall never fall back to any other repository or directory.
- **R12 (constraint):** The git executor shall classify operations:
  **autonomous** = read + local-mutation ops (e.g. `status`, `diff`, `log`,
  `show`, `add`, `commit`, `branch`, `checkout`, `stash`); **gated** = network
  or history-rewriting ops (e.g. `push`, `fetch`/`pull` that write, `reset
  --hard`, `rebase`, force-push, `filter-branch`). Autonomous ops run without
  prompting; gated ops shall require an **explicit per-operation operator
  confirmation** (showing the exact command) before executing.
- **R13 (event):** When a git op executes, the system shall run a real `git`
  subprocess and record the exact argument vector, working directory, exit
  status, and captured stdout/stderr as tool evidence. A non-zero exit is a
  **failure**, not a completion.
- **R14 (constraint):** The git executor shall not run arbitrary shell; it shall
  invoke `git` with an explicit argument vector (no shell interpolation), and
  shall reject any op not on the recognised git-op allow-list.

**i18n**

- **R15 (constraint):** Every new user-facing IDE string introduced by this
  layer (git-confirmation prompt, tool-execution status lines, error messages)
  shall be a `Tr` field translated in all six languages (EN/ES/PT/JA/ZH/FR); no
  hard-coded UI literals.

## 5. Acceptance criteria

- [ ] **AC1 (R1/R2/R3):** A scripted workflow where a specialist emits a
  *declared* tool call executes it and threads the real result into the
  `TaskResult`; an *undeclared*/fabricated tool call fails the task as a critical
  defect with the reason recorded — verified by a `cobolt-ide` test.
- [ ] **AC2 (R4/R5):** A specialist egui inspection call returns a real tree
  census / widget rect from the in-process plugin as evidence; there is no code
  path by which a specialist mutates form design through synthetic events
  (verified by test + code review).
- [ ] **AC3 (R6/R7):** An Approved form-design task's output is applied to the
  target form through `AgentChangeSet`/`AgentPreview`, producing exactly one
  undoable action and the expected control/property changes.
- [ ] **AC4 (R8):** An approved change-set that fails validation leaves the form
  unchanged and marks the task Failed/Blocked with the validation error as
  evidence (test).
- [ ] **AC5 (R9/R10):** The git executor runs against the open project's repo
  path; with no project open (or a non-repo) it errors cleanly and touches no
  other directory — verified in a temp-repo test that also asserts the cwd is the
  project root and never the workspace root.
- [ ] **AC6 (R12):** An autonomous op (`git status`/`commit`) runs without
  prompting; a gated op (`git push`) is not executed until an explicit operator
  confirmation is given, and the exact command is shown in the prompt (test for
  the classifier + review of the confirm UX).
- [ ] **AC7 (R13/R14):** Each executed git op records its argv, cwd, exit status,
  and stdout/stderr as evidence; a non-zero exit marks the task failed; an
  unrecognised git op is rejected (test).
- [ ] **AC8 (R11):** The workflow record and activity log contain one evidenced
  entry per executed tool call with timestamp (test asserts the record shape).
- [ ] **AC9 (R15):** New user-facing strings are present in all six `i18n.rs`
  language tables; `cargo test -p cobolt-ide` passes and the workspace builds.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — git-confirmation prompt, tool-status lines, and
  error messages are new `Tr` fields in all six languages (R15/AC9).
- **Generated-code / regenerate contract:** No change to codegen or the
  regenerate-on-action contract. Form design lands via the form model
  (`AgentChangeSet`), and generated `.cbl` continues to be regenerated on
  Build/Run/Debug/Check as today.
- **Docs (English guide):** The AI-agent / Grace section of
  `docs/developers-guide-en.md` needs an update describing that specialists now
  execute tools (form design applied as a reviewable undoable change; the
  Version Control Agent running git in the open project with local-free /
  network-gated safety). English guide only — translations are user-maintained.
- **Fix vs feature:** **Feature** — this adds new capability (agents executing
  tools). Bumps the **minor** in `crates/cobolt-ide/src/version.rs` + a
  `CHANGELOG.md` entry. NOTE: this rides the `egui-035` branch (spec 027), which
  is not merged; the version bump/CHANGELOG must be reconciled with the spec-027
  T16 merge-gate bump so the two do not collide. Confirm classification with the
  operator at merge time.
- **Branch:** Work continues on `egui-035` (agent-owned; spec 027/028/029 all
  live here). No new branch.
- **Safety:** Network/history-rewriting git ops are operator-gated per R12 — this
  is a *per-project* safety boundary, distinct from the operator's
  PowerRustCOBOL push-window rule (which governs the IDE's own repo).

## 7. Open questions

- **Q1 — Confirmation UX for gated git ops (R12):** modal dialog vs. an inline
  approve/deny control in the Grace progress pane / activity bar? (Resolve in
  `/plan`; either satisfies the requirement.)
- **Q2 — Tool-call transport within a completion (R1):** structured JSON tool
  blocks in the agent reply parsed by the runtime, vs. a provider-native
  tool-calling loop. Leaning toward the reply-parsing contract (consistent with
  Grace's existing plan/verdict JSON contracts and provider-agnostic). Confirm in
  `/plan`.
- **Q3 — Screenshot evidence storage (R4/R11):** store egui screenshots under
  `agentic_ai/Grace/runs/<id>/` and reference them from the record, or keep only
  a textual census? (Design detail for `/plan`.)
- **Q4 — Which egui inspection ops to expose (R4):** the full inspection
  protocol vs. a curated observe-only subset (census + rects + screenshot).
  Recommend the curated subset. Confirm in `/plan`.
