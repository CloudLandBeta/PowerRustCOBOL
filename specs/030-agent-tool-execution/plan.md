<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Runtime tool-execution layer for Grace's specialist agents

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-18

## 1. Approach

The whole layer hangs off one existing seam: the `AgentInvoker` trait in
`cobolt-agents` (`grace.rs:97`). The `GraceEngine` drives every specialist and
reviewer through a single `invoke(agent, system, user) -> Result<String,String>`
call and only reads back a final text submission. We keep that contract — and
therefore keep `cobolt-agents` completely free of IDE/tool concerns — by making
tool execution a **decorator** on the host's invoker.

**Tool execution during a task (R1–R5, R9–R14).** A new
`ToolExecutingInvoker` in `cobolt-ide` wraps the existing `DbAgentInvoker`. Its
`invoke` runs a bounded loop:

1. call the wrapped LLM invoker;
2. parse a trailing fenced JSON tool-call block
   (`{"tool_calls":[{"tool":"…","args":{…}}]}`) from the reply;
3. if none → return the reply as the final submission;
4. otherwise execute each call through a `ToolRegistry`, append the results as a
   new user turn, and loop (bounded round cap).

Governance (R2/R3) lives in the registry: a call naming a tool **not declared**
for the invoking agent (its `tools` / `mcp.json`), a malformed call, or a
reference to a fabricated control/id, returns a *critical-defect* error string
that the engine records as a task failure — never silently completed. Because
every executed result is real backend output threaded back into the reply, "done
without evidence" is structurally impossible for tool work.

**Two tool backends behind the registry:**

- **`egui.*` — observe/verify only (R4/R5).** Reuses `agent_inspection.rs`. The
  worker thread cannot touch `egui::Context` (main-thread only), so observe tools
  read the **cached** inspection snapshot that the main thread refreshes each
  frame while a Grace session is active. Curated subset: `egui.tree` (widget
  census), `egui.rects` (widget rectangles), `egui.screenshot` (saves a PNG
  under the run dir, returns its path). No mutation path exists.
- **`git.*` — the git executor (R9–R14).** A new `git_exec.rs` runs `git` with
  an explicit argument vector (never a shell string), working directory **bound
  to the open project root** (`project_dir`) — never the IDE/workspace repo. An
  op allow-list + classifier splits **autonomous** (status/diff/log/show/add/
  commit/branch/checkout/stash) from **gated** (push, fetch/pull-that-writes,
  reset --hard, rebase, force-push, filter-branch). Gated ops call a confirm
  callback and block until the operator answers. Every run captures argv, cwd,
  exit status, stdout, stderr as evidence; non-zero exit = failure; an
  unrecognised op is rejected.

**Gated-op confirmation across the thread boundary (R12).** `GraceSession`
already streams worker→UI progress over an mpsc channel. We add a **reverse**
channel: on a gated op the worker sends a `GitConfirmRequest{ command }` and
blocks on a one-shot reply; the Grace progress pane renders an inline
Approve / Deny control (non-modal, matching the streaming design) and sends the
answer back. Deny → the op is skipped and the task fails with an evidenced
"operator declined" reason.

**Applying approved form design (R6–R8).** This must run on the main thread
(it mutates the live designer), so it is **not** done in the worker. When
`GraceSession::finished()` yields the record, the app walks the **Approved**
tasks whose agent is the Form Designer, parses each submission as an
`AgentChangeSet` (`agent::parse_change_set`), and applies it to the **originating
designer** via the existing `Designer::apply_agent_change_set` — validation-gated
(`agent::validate`) and pushed as exactly one undoable `Cmd::Batch`. A change-set
that fails validation is not partially applied (R8); the failure is surfaced and
the task marked accordingly. This reuses spec 025's proven path end-to-end; no
new form-write path is introduced.

**Observability (R11).** `ToolExecutingInvoker` accumulates a shared
`Vec<ToolEvidence>` (agent, tool, args-digest, result summary, timestamp).
`grace_host::save_workflow_record` is extended to persist a run file wrapper
`{ "record": WorkflowRecord, "tool_calls": [ToolEvidence] }` (additive — old
run files still parse), and each tool call also emits one `on_progress` line into
the activity log / Grace pane.

## 2. Affected crates / files

- `crates/cobolt-agents/src/grace.rs` — **no change** (deliberate: the engine and
  `AgentInvoker` contract are the seam; keeping this crate pure is a decision, §4).
- `crates/cobolt-ide/src/tool_exec.rs` — **new.** `ToolExecutingInvoker`
  (decorates `DbAgentInvoker`), tool-call JSON parsing, `ToolRegistry`, the
  declared-tools governance check, `ToolCall`/`ToolResult`/`ToolEvidence`, bounded
  tool-round loop.
- `crates/cobolt-ide/src/git_exec.rs` — **new.** Argv git runner (cwd = project
  root), op allow-list, `GitClass::{Autonomous,Gated}` classifier, evidence
  capture, no-repo/no-project errors, unrecognised-op rejection.
- `crates/cobolt-ide/src/agent_inspection.rs` — add the curated observe subset
  (`rects`, `screenshot`) alongside the existing census; expose a worker-safe
  reader over the cached snapshot.
- `crates/cobolt-ide/src/grace_host.rs` — build `ToolExecutingInvoker` instead of
  a bare `DbAgentInvoker`; thread the confirm callback; persist the run-file
  wrapper with tool evidence; `run_grace_workflow` gains a confirm-callback
  parameter.
- `crates/cobolt-ide/src/grace_session.rs` — reverse confirm channel; expose a
  pending `GitConfirmRequest` to the UI + `respond(approved)`.
- `crates/cobolt-ide/src/app.rs` — while a session runs, refresh the inspection
  snapshot each frame; render the inline git Approve/Deny control in the Grace
  pane; on `finished`, apply approved Form-Designer change-sets to the
  originating designer (R7/R8).
- `crates/cobolt-ide/src/llm.rs` — append a machine-readable **git tool-call
  contract** to `DEFAULT_VERSION_CONTROL_PROMPT`, and an **egui observe tool
  contract** to the Form Designer prompt (shared appendix).
- `crates/cobolt-ide/src/agents_db.rs` — declare the concrete tool names
  (`git.*`, `egui.*`) in the seeded agents' `tools` / `mcp.json` so governance
  recognises them (Version Control Agent already carries a git tool label).
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields (×6): git-confirm prompt,
  tool-status lines, tool/gated-op error messages.
- `docs/developers-guide-en.md` — document that specialists now execute tools:
  form design applied as a reviewable undoable change; the Version Control Agent
  running git in the open project with local-free / network-gated safety (English
  only; translations untouched).
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor bump + entry
  (reconcile with the spec-027 T16 merge-gate bump — see §5).

## 3. Data / model changes

- **New types (cobolt-ide):** `ToolCall{ tool, args }`, `ToolResult{ ok,
  summary, detail }`, `ToolEvidence{ agent, tool, args_digest, summary, ts }`,
  `GitClass::{Autonomous,Gated}`, `GitConfirmRequest{ command }`.
- **Run file format:** `agentic_ai/Grace/runs/<id>.json` becomes
  `{ "record": <WorkflowRecord>, "tool_calls": [<ToolEvidence>] }`. Additive and
  back-compatible — a reader that wants only the record can still deserialize the
  `record` field; old files (bare record) are read via a serde fallback.
- **No `.cfrm` / form-model schema change.** Form mutations reuse existing
  `AgentOp`/`AgentChangeSet`; the generated-code contract is untouched.
- **Seed data:** seeded agents' `mcp.json` / `tools` gain explicit tool names;
  existing manifests without them simply expose no tools (safe default).

## 4. Key decisions & alternatives

- **Decision:** Tool execution is a decorator at the `AgentInvoker` seam, in
  `cobolt-ide`. — **Why:** the engine already funnels everything through
  `invoke`; the host is where the `egui::Context`, form model, and `project_dir`
  live; `cobolt-agents` stays a pure, testable logic crate. — **Rejected:**
  teaching `GraceEngine` about tools (couples the pure crate to IDE/egui/git and
  bloats the state machine).
- **Decision:** Tool calls are reply-parsed fenced JSON (resolves spec Q2). —
  **Why:** consistent with Grace's existing plan/verdict/change-set JSON
  contracts and provider-agnostic (works across every configured model/endpoint).
  — **Rejected:** provider-native tool-calling (per-provider wire divergence;
  breaks the uniform `invoke` contract and the Ollama/OpenAI mix already
  supported).
- **Decision:** egui tools are observe-only over the **cached** snapshot; the
  main thread keeps it fresh (resolves spec Q4 → curated subset). — **Why:**
  `egui::Context` is main-thread-only; observe/verify doesn't need synchronous
  round-trips. — **Rejected:** a synchronous main-thread bridge for every observe
  call (more machinery, deadlock surface, no benefit for read-only data).
- **Decision:** Gated git ops confirmed inline in the Grace pane (resolves spec
  Q1). — **Why:** fits the existing streaming-progress UX; no modal to manage
  across the worker boundary. — **Rejected:** blocking modal dialog (heavier,
  fights the async session model).
- **Decision:** Approved form-design change-sets are applied host-side after the
  run, on the main thread, to the originating designer. — **Why:** designer
  mutation is main-thread; reuses `apply_agent_change_set` (validated, undoable).
  — **Rejected:** applying inside the worker/invoker (no `&mut Designer` there;
  would duplicate the apply/undo logic).
- **Decision:** Screenshot evidence saved under `runs/<id>/` and referenced by
  path (resolves spec Q3). — **Why:** keeps the JSON record small; images are
  inspectable artifacts. — **Rejected:** base64 in the record (bloats the JSON).

## 5. Risks & mitigations

- **Risk:** worker touches `egui::Context` → panic/UB. → **Mitigation:** observe
  tools read only the cached snapshot; the context is refreshed exclusively on
  the main thread. Enforced by keeping `agent_inspection`'s context calls
  main-thread-only and giving the worker a snapshot reader.
- **Risk:** confirm bridge deadlocks if the UI never answers (e.g. session
  dismissed). → **Mitigation:** worker blocks on a channel that returns `Deny`
  on disconnect; dismissing the session drops the sender → op treated as declined.
- **Risk:** tool-call loop never terminates (model keeps emitting calls). →
  **Mitigation:** bounded tool-round cap per task (config default, small); on
  exhaustion the task fails with an evidenced reason.
- **Risk:** git executor escaping the project repo. → **Mitigation:** cwd bound
  to `project_dir`; explicit argv only; op allow-list; a test asserts cwd is the
  project root and never the workspace root, and that no-project errors cleanly.
- **Risk:** approved change-set applied to the wrong form. → **Mitigation:** bind
  application to the designer/form that originated the request; skip + report if
  that designer is gone.
- **Risk:** minor-version bump collides with the spec-027 T16 merge-gate bump on
  the same branch. → **Mitigation:** coordinate at merge — one reconciled bump +
  merged CHANGELOG section; flagged in spec §6 and here. Confirm with the operator.
- **Risk:** a network git op runs during the operator's off-hours/push window
  intent. → **Mitigation:** all network/rewrite ops are gated per-op (R12); the
  operator explicitly approves each before it runs.

## 6. Test strategy

Unit/integration (add; each asserts + reports human-readable results):

- **`tool_exec` (cobolt-ide):** a scripted invoker where the agent emits a
  *declared* tool call → executed, result threaded, final submission returned; an
  *undeclared*/fabricated tool call → task-failing critical-defect error; the
  bounded tool-round cap is enforced; `ToolEvidence` is captured per call. (AC1,
  AC8)
- **`git_exec` (cobolt-ide):** against a `tempfile` git repo — classifier splits
  autonomous vs gated correctly; autonomous op runs without a confirm; a gated op
  is not executed until confirmed and the confirm carries the exact command;
  cwd == project root (never workspace root); non-zero exit ⇒ failure evidence;
  unrecognised op rejected; no-project / non-repo errors cleanly. (AC5, AC6, AC7)
- **egui observe:** `egui.tree` returns a census from the cached snapshot; assert
  no mutation entry point exists (compile-time: observe API returns data only).
  (AC2)
- **change-set-on-approved:** an Approved Form-Designer submission is applied via
  `apply_agent_change_set` (one undoable action, expected controls/properties);
  an invalid change-set leaves the form unchanged and marks the task
  Failed/Blocked with the validation error. Reuses the existing
  `apply_agent_change_set` test pattern (designer.rs:10223+). (AC3, AC4)
- **run-file shape:** `save_workflow_record` writes `{record, tool_calls}` and a
  legacy bare-record file still deserializes. (AC8)
- **i18n:** the existing "every `Tr` has 6 languages" test covers the new fields;
  workspace builds + `cargo test -p cobolt-ide` green. (AC9)

Manual / visual (operator-run — I do not drive the app; memory
`never-drive-the-application`):

- In a **scratch COBOL project** with its own git repo: ask Grace (👑) to add a
  control → verify it lands as one undoable change; ask the Version Control Agent
  to commit (runs autonomously) and to push (inline Approve/Deny appears; nothing
  leaves the machine until approved). Confirm the run JSON under
  `agentic_ai/Grace/runs/` carries the tool evidence.

## 7. Steering compliance

- [ ] i18n: all new UI strings (git-confirm prompt, tool-status, errors) in 6
      languages in `i18n.rs`.
- [x] Generated-code banner + regenerate-on-action contract preserved — no
      codegen change; form edits go through the existing model/apply path.
- [ ] English dev guide updated (AI-agent/Grace section); translations untouched.
- [ ] Fix vs feature: **feature** → minor bump in `version.rs` + `CHANGELOG.md`,
      **reconciled with the spec-027 T16 bump** at merge (do not double-bump).
- [x] No "cobolt" in user-facing text; COBOL identifiers/source stay English;
      git executor scoped to the user's project repo, never the IDE's.
