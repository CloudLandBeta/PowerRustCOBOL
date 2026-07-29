<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Agent Progress Transparency (Live Action Status)

- **Status:** done (operator manual check pending)
- **Plan:** ./plan.md   **Date:** 2026-07-28

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

- [x] **T1 — Record model: `action_log` on `WorkflowRecord`** (R11)
  - Files: `crates/cobolt-agents/src/grace.rs`
  - Do: add `ActionLogEntry { agent, kind, detail, at }` (kind = stable
    snake_case string; `at` = epoch millis) and
    `#[serde(default)] pub action_log: Vec<ActionLogEntry>` on
    `WorkflowRecord`. Add serde round-trip tests: an old-format JSON
    (no `action_log`) deserializes to an empty log and re-serializes; a
    populated log round-trips losslessly. Tests report entry counts.
  - Verify: `cargo build -p cobolt-agents` + `cargo test -p cobolt-agents`
    green (covers AC9's data layer).

- [x] **T2 — Typed actions + throttle (`agent_actions.rs`)** (R6, R8)
  - Files: `crates/cobolt-ide/src/agent_actions.rs` (new), `main.rs`/module
    wiring
  - Do: `ActionKind` enum (ReceivingRequest, RetrievingContext, Planning,
    Drafting, RunningTool, Reviewing, ApplyingCorrections, Finishing,
    Blocked, Failed), `AgentAction { agent, kind, detail, at }`,
    kind↔stable-string mapping, and pure `ActionThrottle` over injected
    timestamps (visible line advances ≤1/s, coalesces to latest, drops
    nothing from history). Unit tests: mapping round-trips every variant;
    throttle fed 10 actions in 2 s simulated time asserts ≤1 visible
    transition/second, latest action wins, history holds all 10; reports
    emitted/displayed counts.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide
    agent_actions` green (AC5, AC7 vocabulary shape).

- [x] **T3 — i18n vocabulary ×6** (R9)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: one `Tr` field per `ActionKind` (~10) + `agent_actions_header`
    ("Agent actions") + any new chrome strings, translated in
    EN/ES/PT/JA/ZH/FR; a render-time `ActionKind -> &Tr` lookup in
    `agent_actions.rs`. Extend the i18n tests so every new field is
    non-empty in all six languages.
  - Verify: `cargo test -p cobolt-ide i18n` green (AC8).

- [x] **T4 — Host emission + record attachment** (R1, R2, R5, R7, R11)
  - Files: `crates/cobolt-ide/src/grace_host.rs`
  - Do: add `on_action: &mut dyn FnMut(AgentAction)` to
    `run_grace_workflow*` (no-op helper mirrors `no_progress` for existing
    call sites/tests). Emit at existing boundaries: KB retrieval →
    RetrievingContext; request read/plan → Planning; `TaskStarted` →
    Drafting; tool-evidence flush → RunningTool (verbose: one per tool
    call, named); `ReviewStarted`/`Verdict` → Reviewing;
    `CorrectionRequested` → ApplyingCorrections; `Approved`/`Submitted` →
    Finishing; `Blocked`/`Failed` likewise — each attributed to Grace, the
    task's specialist, or the reviewer. Verbose (`cfg.verbose_log`) adds
    finer-grained entries (per tool call, per review round); actions carry
    **no payloads** in any mode. Before persisting, attach the collected
    actions to `record.action_log`. Integration test on the existing mock
    transport: a two-task run with one review round asserts the action
    sequence, per-agent attribution, and `action_log` == emitted stream;
    reports the sequence.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide
    grace_host` green (AC1 emission, AC6, AC9 attachment).

- [x] **T5 — Session transport + throttled accessor** (R1, R6)
  - Files: `crates/cobolt-ide/src/grace_session.rs`
  - Do: `GraceMsg::Action(AgentAction)`; `GraceSession::actions:
    Vec<AgentAction>` (complete history); wire the worker's `on_action`;
    expose `current_action()` through `ActionThrottle` and the full
    `actions` slice for the collapsed history.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide`
    green (AC1 plumbing, AC5).

- [x] **T6 — Shared chat rendering helpers** (R1, R3, R7, R8)
  - Files: `crates/cobolt-ide/src/panels/editor.rs`
  - Do: `chat_current_action(ui, action, tr, font)` — the throttled
    current-action line in place of the generic "Thinking…" text (reuses
    `chat_thinking_indicator` styling); `chat_action_history(ui, lines,
    tr, font)` — `CollapsingHeader` "Agent actions (N)", collapsed by
    default, one `<agent>: <localized kind> — <detail>` row per action.
    Hardcoded balloon palette only (never `ui.visuals()`); no sizing from
    available/remaining space (egui auto-grow rule).
  - Verify: `cargo build -p cobolt-ide` green; helpers exercised by T7/T8
    surfaces (AC2, AC6 rendering).

- [x] **T7 — Project Grace chat surface** (R1–R6, R10, R11)
  - Files: `crates/cobolt-ide/src/panels/grace_chat.rs`
  - Do: while busy, render `chat_current_action` + live collapsed
    `chat_action_history` instead of the last-8-raw-lines balloon (raw
    `session.log` no longer reaches the pane — payloads stay in the log
    surfaces); `request_repaint_after(1s)` while busy so the line advances.
    On finish, persist one `ChatTurn { role: "actions" }` built from the
    session's action history (supersedes the "Coordination log" markdown
    balloon) and render that role as the collapsed widget on reload;
    legacy histories render unchanged.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide` green;
    code inspection confirms no `session.log` content is rendered in the
    pane (AC1–AC3, AC5 display, AC9 history persistence).

- [x] **T8 — Form-inspector chatbot surface** (R10)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: replace the raw `grace_log` rendering with the same shared helpers
    (current action + collapsed history), identical behaviour to T7.
  - Verify: `cargo build -p cobolt-ide` green; both surfaces call the same
    helpers (AC1/AC2 parity on the second surface).

- [x] **T9 — Action-vs-context boundary audit** (R4, R5)
  - Files: audit across `grace_chat.rs`, `app.rs`, `grace_host.rs`,
    `llm.rs` (no new code expected beyond small fixes)
  - Do: verify no path places retrieved context, tool payloads, reasoning,
    or "Verbose: …" payload lines into either chat pane, in default or
    verbose mode; verbose adds only finer *action* granularity. Confirm the
    full trace still reaches the connection log / diagnostics dump / run
    record. Fix any leak found.
  - Verify: `cargo test -p cobolt-ide` green + a written audit note in the
    task checkoff listing each pane input and its source (AC3, AC4).
  - **Audit note (2026-07-28):**
    - *Project Grace chat pane inputs:* persisted `ChatTurn`s (user text,
      Grace's reply balloons, `"actions"` turns rendered as the collapsed
      widget); the live typed action stream (`GraceSession::actions`, details
      are task ids / tool names / round numbers only); the throttled
      current-action line; the git-approval block (spec 030 UI, operator
      gate — not progress). The raw `session.log` is no longer rendered
      (previously it showed the last 8 lines, including verbose payloads).
    - *Form-inspector chatbot inputs:* the same collapsed history + current
      action helpers; the finished-run status line (workflow id/status/
      record path); the change-set preview (spec 025 approval UI). Raw
      `grace_log` lines removed.
    - *Emission sites:* every `AgentAction.detail` carries a task id, tool
      name, `"plan, round n/3"`, or workflow id — no payloads; verbose adds
      only `Submitted` actions and per-round review detail
      (`action_for_event`, `flush_tools`).
    - *Full trace homes:* all `on_progress` lines now also flow to the AI
      log (Output panel) via `push_ai_log(Detail)` in `GraceSession::spawn`;
      verbose payloads keep going to `push_ai_log`/`push_connection_log`;
      the run record persists `action_log` + tool evidence.
    - *Out of plan scope (pre-existing, flagged):* the compact contextual
      chats (editor/designer inline chats via `spawn_contextual_request`)
      still stream progress lines transiently as reply chunks, and the
      finished-run verbose reply balloons (spec 029 behaviour) still show
      specialist submissions in the transcript — both are existing features
      outside this spec's two surfaces; follow-up candidate.

- [x] **T10 — Docs (English guide)** 
  - Files: `docs/developers-guide-en.md` (translations untouched)
  - Do: update the Grace/AI-assistant section: live action lines, collapsed
    action history, per-agent attribution, default vs verbose granularity,
    the action-vs-context boundary, and where full traces live (connection
    log, diagnostics dump, `agentic_ai/Grace/runs/`).
  - Verify: guide section reads accurately against the implemented UI;
    no other guide languages modified.

- [x] **T11 — Finalize** (feature) — code/tests/docs complete; the manual
  UI checklist below remains with the operator.
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump minor → **1.38.0**; CHANGELOG entry (note the verbose-payload
    behaviour change in the chat pane). Full `cargo build` + `cargo test`
    across touched crates (`cobolt-ide`, `cobolt-agents`).
  - Verify: all builds/tests green; operator manual check per plan §6
    (changing action line ≥1 s apart; expandable attributed history; no
    payloads verbose on/off; language switch re-localizes; reopen project →
    history reviewable; repeat in form inspector). Feature-only commit on
    `feat/agent-progress-transparency`; no push without the operator.

## Done criteria

All acceptance criteria in spec.md are checked (AC1–AC9 map to the task
verifications above), tests pass, docs updated, and the change is a single
feature commit series on `feat/agent-progress-transparency` per the operator's
rules (do **not** commit/push unless the operator asks).
