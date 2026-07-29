<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Agent Progress Transparency (Live Action Status)

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-07-28

## 1. Approach

Today `grace_host::run_grace_workflow` streams **untyped strings** through
`on_progress` into `GraceSession::log`; the project Grace chat renders the
last 8 raw lines as a balloon, and the form-inspector chatbot renders
`grace_log` similarly. Those strings mix task transitions, tool evidence,
Knowledge Base notes, and — under verbose — **payloads** ("Verbose: Loaded
Skills: …"), which violates the spec's action-vs-context boundary (R4).

The design introduces a **typed action stream** parallel to the existing
string log, and makes the chat surfaces render *only* the typed stream:

1. **Typed actions** (R1, R2, R7, R8). New module
   `crates/cobolt-ide/src/agent_actions.rs`:
   - `ActionKind` — closed enum of canonical steps: `ReceivingRequest`,
     `RetrievingContext`, `Planning`, `Drafting`, `RunningTool`,
     `Reviewing`, `ApplyingCorrections`, `Finishing`, `Blocked`, `Failed`.
   - `AgentAction { agent: String, kind: ActionKind, detail: String,
     at: SystemTime }` — `agent` is "Grace" or the specialist/reviewer name
     from the `TaskSpec` (attribution, R7); `detail` is a short dynamic
     fragment (task id, control name, tool name), interpolated verbatim.
   - Emission points in `grace_host.rs` map 1:1 onto boundaries that already
     exist: the KB retrieval block → `RetrievingContext`; "Grace is reading
     the request…" / plan production → `Planning`; `GraceEvent::TaskStarted`
     → `Drafting`; tool-evidence flush → `RunningTool`;
     `ReviewStarted`/`Verdict` → `Reviewing`; `CorrectionRequested` →
     `ApplyingCorrections`; `Approved`/`Submitted` → `Finishing`;
     `Blocked`/`Failed` likewise. `run_grace_workflow_with_control` gains an
     `on_action: &mut dyn FnMut(AgentAction)` callback next to
     `on_progress`; the string log is unchanged (it remains the full trace).
2. **Session transport** (R1, R6). `grace_session.rs`: new
   `GraceMsg::Action(AgentAction)` variant; `GraceSession` accumulates
   `pub actions: Vec<AgentAction>` (complete, unthrottled — the persisted
   history) and exposes the throttled current line via a small pure
   `ActionThrottle` helper (injected clock, unit-testable): the *visible*
   current-action line changes at most once per second; faster actions are
   coalesced by skipping straight to the latest when the window elapses.
   Nothing is dropped from `actions` (R6).
3. **Rendering** (R1–R3, R5, R7, R8, R10). Shared helpers in
   `panels/editor.rs` beside the existing `chat_bubble*` family:
   - `chat_action_history(ui, actions, tr, font)` — an
     `egui::CollapsingHeader` ("Agent actions (N)", collapsed by default)
     listing every action line, each rendered as
     `<agent>: <localized kind> — <detail>` (R3, R7).
   - `chat_current_action(ui, action, tr, font)` — replaces the generic
     "Thinking…" label of `chat_thinking_indicator` with the throttled
     current action while a session runs (R1).
   Both surfaces — `panels/grace_chat.rs` and the form-inspector chatbot in
   `app.rs` — switch from rendering raw `session.log` lines to these helpers
   (R10). Colors come from the hardcoded balloon palette, never
   `ui.visuals()` (glass-theme contrast rule). Verbose mode (`verbose_log`,
   reused per spec) adds the finer-grained entries (per tool call, per
   review round) to the *action* stream; payload-bearing "Verbose: …" lines
   stay in the string log only and no longer reach the chat pane (R4, R5).
4. **Persistence** (R3, R11). Two layers:
   - `cobolt-agents::grace::WorkflowRecord` gains
     `#[serde(default)] pub action_log: Vec<ActionLogEntry>` where
     `ActionLogEntry { agent, kind, detail, at }` stores `kind` as a stable
     snake_case string. The host attaches the collected actions before the
     record is persisted to `agentic_ai/Grace/runs/<id>.json`. Old records
     without the field still deserialize (`serde(default)`).
   - On finish, the chat persists one `ChatTurn` with the new role
     `"actions"` (content = serialized action lines), which both surfaces
     render as the collapsed history; this replaces today's "Coordination
     log" markdown balloon in `grace_chat::poll`. Reopening the project
     reloads it via the existing `persist()`/`history_path` JSON (R11).
     Legacy histories (plain `assistant` balloons) render unchanged.
5. **Localization** (R9). `kind` → text happens **at render time** through
   new `Tr` fields (one per `ActionKind`, plus the collapsed-header label),
   translated in all six languages. Because records store the stable kind
   string and raw detail, a run recorded under one IDE language re-localizes
   correctly when reviewed under another; dynamic details stay verbatim
   (resolved i18n decision).
6. **Full traces keep their home** (R4). The string log (`session.log`)
   remains the complete trace: it still feeds the LLM connection log /
   debug-settings diagnostics and the saved workflow record. The chat pane
   simply stops being a raw-log viewer.

## 2. Affected crates / files

- `crates/cobolt-agents/src/grace.rs` — `ActionLogEntry`,
  `WorkflowRecord.action_log` (`serde(default)`).
- `crates/cobolt-ide/src/agent_actions.rs` — **new**: `ActionKind`,
  `AgentAction`, `ActionThrottle`, kind↔stable-string mapping.
- `crates/cobolt-ide/src/grace_host.rs` — emit `AgentAction`s at the
  existing boundaries; attach `action_log` to the record before persisting;
  `run_grace_workflow*` signatures gain `on_action`.
- `crates/cobolt-ide/src/grace_session.rs` — `GraceMsg::Action`,
  `GraceSession::actions`, throttled current-action accessor.
- `crates/cobolt-ide/src/panels/editor.rs` — shared `chat_action_history` /
  `chat_current_action` helpers (balloon palette).
- `crates/cobolt-ide/src/panels/grace_chat.rs` — replace the last-8-raw-lines
  balloon; render collapsed history + current action; persist the
  `"actions"` turn on finish (supersedes the "Coordination log" balloon).
- `crates/cobolt-ide/src/app.rs` — form-inspector chatbot: same helpers in
  place of raw `grace_log` rendering.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6 (EN/ES/PT/JA/ZH/FR):
  one per `ActionKind` (~10) + `agent_actions_header` + any chrome.
- `crates/cobolt-ide/src/version.rs` — minor bump (1.37.x → 1.38.0).
- `CHANGELOG.md` — feature entry.
- `docs/developers-guide-en.md` — Grace/AI-assistant section: default vs
  verbose status lines, action-vs-context boundary, where full traces live.

## 3. Data / model changes

- **`WorkflowRecord` (on-disk JSON, `agentic_ai/Grace/runs/`):** additive
  `action_log` array; `#[serde(default)]` keeps every existing record
  readable. `kind` is a stable snake_case string (not the localized text) so
  records are language-independent.
- **Chat history JSON (`ChatTurn`):** new `role: "actions"` value.
  `role` is already a free `String`, so the format does not change; old
  readers of the file (none outside the IDE) are unaffected, and old
  histories render as before.
- No `.cfrm`, codegen, or project-file changes.

## 4. Key decisions & alternatives

- **Typed parallel action channel** next to the string log — Why: strings
  can't carry attribution or survive localization; parsing them back is
  fragile. — Rejected: regex-classifying `session.log` lines in the UI.
- **Display-side throttle, lossless history** — Why: R6 forbids losing
  actions; throttling at emission would drop them. The visible line coalesces
  by jumping to the latest action each 1 s window; the collapsed list gets
  everything. — Rejected: emit-side rate limiting; per-line animation queues
  (complexity, no requirement).
- **`action_log` embedded in `WorkflowRecord`** — Why: the record is already
  the run's single persisted audit file; `serde(default)` gives free
  backward compat. — Rejected: sidecar file per run (two files to keep in
  sync); chat-history-only persistence (loses the record's reviewability).
- **New `"actions"` ChatTurn role** rendered as a collapsing widget — Why:
  lets the persisted history reopen collapsed (R3/R11) with zero schema
  migration. — Rejected: markdown-only balloon (can't collapse; replays as a
  wall of text).
- **Render-time localization from stable kind strings** — Why: honours the
  resolved i18n decision and keeps saved records language-neutral. —
  Rejected: storing localized text in the record (locks the record to the
  language it ran under).
- **Reuse `LlmConfig::verbose_log`** — spec mandate; no new toggle.

## 5. Risks & mitigations

- **Behaviour change:** verbose payload lines ("Verbose: Loaded Skills…")
  disappear from the chat pane (they were visible there today). → This is
  exactly R4; call it out in the CHANGELOG and the guide (full traces: LLM
  connection log / diagnostics dump / run record).
- **egui self-inflation:** the collapsing history lives inside the existing
  `ScrollArea` with `stick_to_bottom`; helpers must never size themselves
  from available/remaining space (project golden rule). → Fixed-row line
  rendering; reuse the proven `chat_bubble*` layout; no `Resize` containers.
- **Glass-theme contrast:** status lines could inherit dark-on-dark from
  `ui.visuals()`. → Hardcode the balloon palette like the thinking
  indicator already does.
- **Two surfaces drifting:** grace_chat and the form-inspector chatbot
  duplicating rendering. → Single shared helpers in `panels/editor.rs`;
  both call the same functions (R10).
- **Signature churn:** `run_grace_workflow*` is called from tests and two
  spawn paths. → Additive parameter with a no-op default helper
  (mirroring the existing `no_progress` pattern) keeps test churn local.
- **Throttle jitter across frames:** egui repaints are irregular. →
  `ActionThrottle` is pure over injected timestamps; the pane calls
  `request_repaint_after(1s)` while busy so the line advances without user
  input.

## 6. Test strategy

- `cobolt-agents` (unit): `WorkflowRecord` serde round-trip **with** and
  **without** `action_log` — asserts an old-format JSON (no field)
  deserializes and re-serializes; reports the entry counts read/written.
- `cobolt-ide` (unit, `agent_actions.rs`):
  - Event→kind mapping covers **every** `GraceEvent` variant (compile-time
    exhaustive match + assertions on agent attribution and detail);
    reports the mapped table.
  - `ActionThrottle`: feed 10 actions in 2 s of simulated clock → asserts
    ≤1 visible transition per second, final visible line is the latest
    action, and the full history still holds all 10; reports
    emitted/displayed counts.
  - Kind↔stable-string mapping round-trips for every variant.
- `cobolt-ide` (unit, i18n): every new `Tr` action field is non-empty in
  all six languages (same pattern as existing i18n tests).
- `cobolt-ide` (integration, `grace_host` mock transport — reuse the
  existing mock-workflow tests): a two-task run with one review round
  asserts the `on_action` sequence (`Planning → Drafting → Reviewing →
  Finishing`…), correct per-agent attribution, and that the persisted
  record's `action_log` matches the emitted stream; reports the sequence.
- **Build+test gate:** `cargo build -p cobolt-ide` and
  `cargo test -p cobolt-ide -p cobolt-agents` must pass.
- **Manual/visual (operator):** launch the IDE, run a long Grace request in
  the project chat: (1) a changing action line replaces "Thinking…" while
  busy, ≥1 s apart; (2) expanding "Agent actions (N)" shows the ordered,
  attributed steps; (3) no retrieved context/payloads appear in the pane,
  verbose on or off; (4) verbose on shows finer steps; (5) switch IDE
  language → canonical vocabulary re-localizes; (6) reopen the project →
  the collapsed history is still reviewable; (7) repeat once in the form
  inspector chatbot. (Per project rule, the operator verifies the UI —
  Claude verifies via build/tests only.)

## 7. Steering compliance

- [ ] i18n: all new UI strings (`ActionKind` vocabulary + header) as `Tr`
  fields in all 6 languages; dynamic fragments verbatim (per resolved spec
  decision).
- [ ] Generated-code banner + regenerate-on-action contract: untouched (no
  codegen changes).
- [ ] English dev guide updated; translations untouched (user-maintained).
- [ ] Fix vs feature: **feature** → minor bump to 1.38.0 in `version.rs` +
  `CHANGELOG.md` entry; branch `feat/agent-progress-transparency`; no
  fix/feature mixing in commits.
- [ ] No "cobolt" in user-facing text; COBOL identifiers stay English.
