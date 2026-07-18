<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Runtime tool-execution layer for Grace's specialist agents

- **Status:** done (code T1–T13 complete & green; T14 version/CHANGELOG deferred to spec-027 T16 merge gate per branch convention)
- **Plan:** ./plan.md   **Date:** 2026-07-18

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. The workspace stays green
after every task. `cobolt-agents` is intentionally **not** modified.

---

- [x] **T1 — Tool-call & evidence types + parser** (R1, R2, R3, R11)
  - Files: `crates/cobolt-ide/src/tool_exec.rs` (new); `crates/cobolt-ide/src/lib.rs`/`main.rs` (add `mod tool_exec;`).
  - Do: define `ToolCall{ tool, args }`, `ToolResult{ ok, summary, detail }`,
    `ToolEvidence{ agent, tool, args_digest, summary, ts }`. Add
    `parse_tool_calls(reply) -> Option<Vec<ToolCall>>` that extracts a trailing
    fenced JSON `{"tool_calls":[…]}` block (reuse the `agent::extract_json`
    fence/brace convention). No execution yet.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide tool_exec::`
    — unit tests: a reply with a valid block parses to N calls; a reply with no
    block → `None`; a malformed block → parse error (not a silent empty).

- [x] **T2 — Tool registry + declared-tools governance** (R1, R2, R3)
  - Files: `crates/cobolt-ide/src/tool_exec.rs`.
  - Do: `ToolRegistry` mapping tool name → backend fn, built from the invoking
    agent's declared `tools`/`mcp.json`. `dispatch(agent, call) -> ToolResult`:
    an **undeclared/unknown** tool, a malformed call, or (backends later) a
    fabricated id returns a *critical-defect* error result. Add a
    `CRITICAL_DEFECT` marker string the invoker turns into a task failure.
  - Verify: `cargo test -p cobolt-ide tool_exec::` — declared tool dispatches;
    undeclared tool → critical-defect result; assert the governance message text.

- [x] **T3 — `ToolExecutingInvoker` bounded loop** (R1, R2, R3, R11)
  - Files: `crates/cobolt-ide/src/tool_exec.rs`.
  - Do: struct wrapping any `AgentInvoker` + a `ToolRegistry` + shared
    `Arc<Mutex<Vec<ToolEvidence>>>` + a `max_tool_rounds` cap. `impl AgentInvoker`:
    call inner → parse tool calls → if none, return reply; else execute, record
    evidence, append a results turn to `user`, loop; a critical-defect result
    ends the task with an `Err`; exceeding the cap ends with an evidenced `Err`.
  - Verify: `cargo test -p cobolt-ide tool_exec::` with a **scripted inner
    invoker**: (a) declared call → executed, result threaded, final reply
    returned, one `ToolEvidence` captured; (b) undeclared call → `Err`
    (task-failing); (c) loop cap enforced. **(AC1, AC8)**

- [x] **T4 — Git executor: runner, scope, classifier** (R9, R10, R12, R13, R14)
  - Files: `crates/cobolt-ide/src/git_exec.rs` (new); register `mod git_exec;`.
  - Do: `GitClass::{Autonomous,Gated}`; `classify(argv)` over an op allow-list
    (autonomous: status/diff/log/show/add/commit/branch/checkout/stash…; gated:
    push, fetch/pull-that-writes, reset --hard, rebase, force-push,
    filter-branch); unrecognised op → rejected. `run_git(project_dir, argv)`:
    explicit argv (no shell), cwd = `project_dir`, capture argv/cwd/exit/stdout/
    stderr into evidence; non-zero exit ⇒ failure. Clean error when
    `project_dir` is absent or not a git repo.
  - Verify: `cargo test -p cobolt-ide git_exec::` against a `tempfile` repo —
    classifier autonomous vs gated vs rejected; `run_git` cwd **== project root,
    never workspace root**; non-zero exit ⇒ failure evidence; no-project / non-repo
    errors cleanly. **(AC5, AC7)**

- [x] **T5 — Gated-op confirm bridge** (R12)
  - Files: `crates/cobolt-ide/src/grace_session.rs`; `crates/cobolt-ide/src/grace_host.rs`; `crates/cobolt-ide/src/tool_exec.rs`.
  - Do: add a reverse channel — `GitConfirmRequest{ command }` + one-shot reply.
    Worker blocks on a confirm callback for gated ops; **Deny (incl. channel
    disconnect) → op skipped, task fails with an evidenced "operator declined"**.
    Thread a `confirm: &mut dyn FnMut(GitConfirmRequest) -> bool` through
    `run_grace_workflow` into the git backend.
  - Verify: `cargo test -p cobolt-ide` — a gated op is not executed until the
    callback returns true; a false/disconnected callback yields the declined
    failure; the request carries the **exact command**. **(AC6)**

- [x] **T6 — Wire git backend into the registry** (R2, R9, R12, R13, R14)
  - Files: `crates/cobolt-ide/src/tool_exec.rs`; `crates/cobolt-ide/src/git_exec.rs`.
  - Do: register `git.*` tools; map a git `ToolCall` → `git_exec` (autonomous run
    directly; gated via the confirm bridge from T5); fabricated/unknown git op →
    critical defect. Only the Version Control Agent (declares `git.*`) can reach it.
  - Verify: `cargo test -p cobolt-ide` — VC agent git call executes/records; a
    non-declaring agent calling `git.*` → critical defect. **(AC7)**

- [x] **T7 — egui observe tools (cached snapshot)** (R4, R5)
  - Files: `crates/cobolt-ide/src/agent_inspection.rs`; `crates/cobolt-ide/src/tool_exec.rs`.
  - Do: extend `agent_inspection` with a worker-safe reader over the cached
    snapshot exposing `egui.tree` (census), `egui.rects`, `egui.screenshot`
    (save PNG under `runs/<id>/`, return path). Register as **observe-only**
    tools; keep all `egui::Context` calls main-thread-only (no mutation path).
  - Verify: `cargo test -p cobolt-ide` — `egui.tree` returns a census from a
    seeded cached snapshot; observe API returns data only (no mutation entry
    point — code review + type check). **(AC2)**

- [x] **T8 — grace_host: use `ToolExecutingInvoker` + persist tool evidence** (R1, R11)
  - Files: `crates/cobolt-ide/src/grace_host.rs`.
  - Do: build `ToolExecutingInvoker` (wrapping `DbAgentInvoker`) with a registry
    scoped per invoked agent; after the engine returns, save the run file as
    `{ "record": WorkflowRecord, "tool_calls": [ToolEvidence] }` (additive; keep
    a serde fallback that reads a legacy bare-record file). Emit one
    `on_progress` line per tool call.
  - Verify: `cargo test -p cobolt-ide grace_host::` — run file has both fields;
    a legacy bare-record JSON still deserializes; progress lines include tool
    entries. **(AC8, AC11 via record shape)**

- [x] **T9 — Tooling contracts in prompts + seeded tool declarations** (R2)
  - Files: `crates/cobolt-ide/src/llm.rs`; `crates/cobolt-ide/src/agents_db.rs`.
  - Do: append the machine-readable **git tool-call contract** to
    `DEFAULT_VERSION_CONTROL_PROMPT` and the **egui observe contract** to the Form
    Designer prompt (shared appendix). Declare concrete tool names (`git.*`,
    `egui.*`) in the seeded agents' `tools`/`mcp.json` so governance recognises
    them. Keep the VC prompt's existing scope/safety wording (project repo only).
  - Verify: `cargo test -p cobolt-ide` (seed/agents_db tests) — seeded VC agent
    declares `git.*`; Form Designer declares `egui.*`; prompt strings contain the
    contract; `ensure_*` idempotent.

- [x] **T10 — Apply approved form-design change-sets host-side** (R6, R7, R8)
  - Files: `crates/cobolt-ide/src/app.rs`.
  - Do: when `GraceSession::finished()` yields the record, walk **Approved**
    Form-Designer tasks, `parse_change_set` each submission, and apply to the
    **originating** designer via `apply_agent_change_set` (validated, one undoable
    action). Invalid change-set → nothing applied, surface the error, mark handled;
    skip if the originating designer is gone. No new form-write path.
  - Verify: `cargo test -p cobolt-ide` — approved submission applies (expected
    controls/props, single undo step); invalid change-set leaves the form
    unchanged with the validation error surfaced. **(AC3, AC4)**

- [x] **T11 — Grace pane: inline git Approve/Deny + snapshot refresh** (R4, R12, R15)
  - Files: `crates/cobolt-ide/src/app.rs`; `crates/cobolt-ide/src/i18n.rs`.
  - Do: while a session runs, refresh the inspection snapshot each frame; render
    an inline Approve/Deny control showing the exact command when a
    `GitConfirmRequest` is pending, wired to `GraceSession::respond`. All strings
    via new `Tr` fields.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide`; operator
    visual check deferred to T13 (I do not drive the app).

- [x] **T12 — i18n: new Tr keys ×6 languages** (R15)
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do: add every new user-facing string (git-confirm prompt/labels, tool-status
    lines, tool + gated-op errors) as `Tr` fields in all six languages
    (EN/ES/PT/JA/ZH/FR). No hard-coded literals in T11.
  - Verify: `cargo test -p cobolt-ide i18n` — no empty translations; workspace
    builds. **(AC9)**

- [x] **T13 — Docs (English guide)** (spec §6)
  - Files: `docs/developers-guide-en.md`.
  - Do: update the AI-agent/Grace section: specialists now execute tools — form
    design applied as a reviewable, undoable change; the Version Control Agent
    running git **in the open project** with local-free / network-gated safety;
    tool evidence in run records. English only — translations untouched.
  - Verify: section renders in the in-app doc viewer; no translation files edited
    (`git status` shows only `-en.md`).

- [x] **T14 — Finalize** (all AC)
  - Files: `crates/cobolt-ide/src/version.rs`; `CHANGELOG.md`.
  - Do: **version bump + CHANGELOG entry are DEFERRED to spec-027 T16** — the
    established `egui-035` branch convention (confirmed: `git log main..egui-035`
    touches neither file; specs 027/028/029 all reserve it for the merge gate) is
    a single reconciled feature entry at merge, NOT a per-spec bump. Doing it here
    would double-bump. Full workspace build + test done instead.
  - Verify: ✓ `cargo build` (workspace) clean; ✓ `cargo test` workspace green —
    `cobolt-ide` 188 passed / 0 failed (+21 new), `cobolt-agents` 13 passed / 0
    failed. Every spec AC1–AC9 covered by a passing test above. Operator manual
    check per plan §6 pending (scratch project: add-control undoable change;
    commit autonomous; push gated → confirm; run JSON carries tool evidence).
    Not committed/pushed — awaiting operator.

## Done criteria
All acceptance criteria in spec.md (AC1–AC9) are checked by a task's
verification, the full workspace builds and tests green, the English guide is
updated (translations untouched), and the change is a single **feature** commit
set with a version/CHANGELOG bump reconciled against spec-027 T16. Do **not**
commit or push unless the operator asks (respect the push-window rule).
