<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — AI Development Agent (dev-time form-building assistant)

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-07-08

Ordered, small, independently-verifiable tasks. Logic-first (unit-testable, keeps
the crate green), then wiring, then UI (compiles + manual checks), then docs/i18n/
finalize.

- [x] **T1 — Change-set types + JSON parsing** (R3, R4)
  - Files: `crates/cobolt-ide/src/agent.rs` (new), `crates/cobolt-ide/src/main.rs`
    or lib module list (register `mod agent`).
  - Do: define `AgentChangeSet { operations: Vec<AgentOp>, note: Option<String> }`
    and `AgentOp` (`DeployControl`, `SetProperty`, `GenerateEventHandler`,
    `CreateProcedure`) with `serde` deserialisation matching the wire format in
    plan §3. Add `parse_change_set(&str) -> Result<AgentChangeSet, String>` that
    extracts the ```json fence and deserialises.
  - Verify: `cargo test -p cobolt-ide agent::` — parses one well-formed change-set
    with all four op kinds; malformed / unknown-`op` JSON returns `Err`
    (nothing applied). Covers **AC10**.

- [x] **T2 — Validate ops against the live schema** (R9)
  - Files: `crates/cobolt-ide/src/agent.rs`; read-only use of
    `cobolt_forms::model::{ControlType, property_names_for}` + `supported_events`.
  - Do: `validate(&AgentChangeSet, &Form) -> Vec<OpStatus>` flagging unknown
    `control_id`, unsupported `control_type`, unknown property `key`, and
    unsupported `event` as errors; healthy ops as ok.
  - Verify: `cargo test -p cobolt-ide agent::validate` — sample form: unknown id /
    bad type / bad key / bad event each flagged; valid ops pass. Covers **AC4**.

- [x] **T3 — Context builder** (R2, R4)
  - Files: `crates/cobolt-ide/src/agent.rs`.
  - Do: `build_context(&Form) -> String` — compact inventory (id, type, non-default
    props), per-type property legend (`property_names_for`), per-type event legend
    (`supported_events`), and existing `user_procedures` names.
  - Verify: `cargo test -p cobolt-ide agent::context` — emitted context contains a
    sample form's control ids/types and the legends.

- [x] **T4 — Default assets + non-destructive scaffold + resolvers** (R18, R19, R21)
  - Files: `crates/cobolt-ide/src/agent.rs`; embed
    `include_str!("../../../specs/025-ai-dev-agent/agent-system-prompt.md"→ constant
    body)` and the skill — **copy** the prompt body + skill into
    `crates/cobolt-ide/src/assets/` (do not `include_str!` from `specs/`); path
    helpers.
  - Do: `AGENT_SYSTEM_PROMPT` const; `ensure_agentic_ai_scaffold(project_dir)`
    (create `agentic_ai/` + `skills/`, write only **missing** `system-prompt.md`
    and `skills/rustcobol-extensions.md`, never overwrite); `effective_prompt(dir)`
    (file if non-empty else const); `load_skills(dir) -> String`.
  - Verify: `cargo test -p cobolt-ide agent::scaffold` (tmpdir) — empty dir →
    creates folder + both files; re-run = no-op; edited `system-prompt.md` left
    untouched while a deleted skill is re-seeded; `effective_prompt` returns file
    then falls back to const. Covers **AC16, AC17, AC20**.

- [x] **T5 — Wire scaffold into project create + open** (R19)
  - Files: `crates/cobolt-ide/src/app.rs` (create-project and open-project actions).
  - Do: call `agent::ensure_agentic_ai_scaffold(project_dir)` in both paths.
  - Verify: `cargo build -p cobolt-ide`; launch, open a project **without**
    `agentic_ai/` → folder + `system-prompt.md` + `skills/rustcobol-extensions.md`
    appear; create a new project → same. Covers **AC16, AC17**.

- [x] **T6 — New undoable Cmd variants** (R6)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`; if needed
    `crates/cobolt-forms/src/model.rs` (`Control::set_event_code`,
    `Form::add_procedure`).
  - Do: add `Cmd::SetEventCode { control_id, event, old, new }`,
    `Cmd::AddProcedure { name, old, new }`, and `Cmd::AgentBatch(Vec<Cmd>)` with
    do (in order) / undo (reverse) in the existing `apply`/`undo`/`redo` handlers.
  - Verify: `cargo test -p cobolt-ide designer::agent_batch` — apply an
    `AgentBatch` (add control + set property + set event code + add procedure) then
    undo restores the `Form` byte-identically; redo re-applies. Covers **AC2, AC3,
    AC5, AC6, AC8**.

- [x] **T7 — Change-set → Cmd batch** (R4, R6, R13)
  - Files: `crates/cobolt-ide/src/agent.rs` + `designer.rs`.
  - Do: `to_agent_batch(&AgentChangeSet, &Form) -> Cmd::AgentBatch` — deploy→
    `AddControl` (default non-overlapping geometry when omitted), set_property→
    `SetProperty`, handler→`SetEventCode`, procedure→`AddProcedure`; skip/exclude
    invalid ops (from T2).
  - Verify: `cargo test -p cobolt-ide agent::to_batch` — a mixed change-set yields
    the expected `Cmd` children; a control without geometry gets defaults (R13).

- [x] **T8 — Agent LLM request + memory** (R2, R12, R14, R15, R16, R21)
  - Files: `crates/cobolt-ide/src/llm.rs`, `agent.rs`.
  - Do: `compose_agent_request(cfg, dir, form, history, prompt)` — system =
    `effective_prompt(dir)`; append `load_skills(dir)` as system content; replay
    `history`; final user turn = prompt + `build_context(form)`. Reuse
    `spawn_request` transport. Persist only `user`/`assistant` turns via
    `save_history` under a **dev-agent key** (`agent-history-<form-id>`); never pass
    prompt/skills/context to `save_history`.
  - Verify: `cargo test -p cobolt-ide llm::agent_compose` — composed messages start
    with the effective prompt + skills and end with prompt+context; the persisted
    history contains only turns (no prompt/skills/context). Covers **AC12, AC14,
    AC19** (composition side).

- [x] **T9 — Preview model + Approve/Reject apply routing** (R5, R6, R7)
  - Files: `crates/cobolt-ide/src/app.rs` (owns transient change-set/preview state
    next to `Form` + designer), `agent.rs`.
  - Do: hold `Option<AgentPreview>` (validated ops + statuses); **Approve** →
    `designer.apply(to_agent_batch(..))` (one undo step) then clear; **Reject** →
    clear, mutate nothing. Nothing is applied before Approve.
  - Verify: `cargo build -p cobolt-ide`; unit test that Approve produces exactly one
    `AgentBatch` on the undo stack and Reject leaves the stack empty. Covers **AC8**
    (+ AC2/AC3/AC5/AC6 apply path).

- [x] **T10 — Agent prompt-bar + preview UI** (R1, R5, R7, R8, R11, R12)
  - **Placement change (approved):** built at the **form inspector** (`app.rs`), not
    the generic `editor.rs::ai_bar` — only the inspector has the live `Form`. Replaces
    the inspector's read-only generated-COBOL assistant.
  - Files: `crates/cobolt-ide/src/app.rs` (`agent_bar` + `agent_op_line` + state),
    `i18n.rs` (10 `Tr` keys ×6, verified by i18n tests).
  - Do: Code↔Agent mode toggle (agent mode only when `LlmConfig::is_configured()`);
    on agent reply parse→validate→open the **preview panel** (op list, before→after,
    generated code, error ops) with **Approve**/**Reject**; in-flight "thinking…"
    + **cancel**; error surfaced, applies nothing; send only on explicit submit (no
    timers/auto).
  - Verify: `cargo build -p cobolt-ide`; launch — bar hidden when unconfigured
    (**AC1**); "add a Save button" → preview → Approve adds one Button, one Undo
    removes it (**AC2**); "make Label1 green" (**AC3**); handler/procedure
    (**AC5/AC6**); unknown id shows error op (**AC4**); Reject unchanged (**AC8**);
    cancel in-flight (**AC11**); no action without submit (**AC7**).

- [ ] **T11 — Settings: modal agent-prompt editor** (R17)
  - Files: `crates/cobolt-ide/src/panels/settings_form.rs`, `i18n.rs`.
  - Do: "Edit agent prompt…" button in the AI section opens a modal editor
    (multiline) loading `agentic_ai/system-prompt.md` (default text when absent);
    **Save** writes the file, **Reset to default** rewrites `AGENT_SYSTEM_PROMPT`,
    **Cancel** discards.
  - Verify: `cargo build -p cobolt-ide`; launch — edit + Save persists to the file;
    next agent request uses it; Reset restores default; Cancel discards. Covers
    **AC15**.

- [ ] **T12 — Project tree "Agentic AI" category** (R20)
  - Files: `crates/cobolt-ide/src/panels/project.rs`, `app.rs`, `i18n.rs`.
  - Do: add `Category::AgenticAi` + `show_category` branch (icon + list of
    `agentic_ai/**` files) + `ProjectPanelEvent::OpenAgenticFile(PathBuf)`;
    `app.rs` opens the file in the editor viewport in plain-text mode (no COBOL
    codegen/regenerate). Any file editable + saveable.
  - Verify: `cargo build -p cobolt-ide`; launch — "Agentic AI" node appears level
    with Forms/Indexed Files; lists the files; double-click opens + edit + save
    round-trips. Covers **AC18**.

- [ ] **T13 — Docs & i18n** (R11)
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`.
  - Do: add an "AI development agent" section (configure via AI settings, prompt
    bar agent mode, preview/approve, undo, the editable prompt, the `agentic_ai/`
    folder + tree node, RustCOBOL skill). Add every new `Tr` key in all six
    languages (agent mode, prompt hint, preview labels/op names, before→after,
    Approve/Reject, thinking, errors, "Edit agent prompt…", modal title/Save/Reset/
    Cancel, "Agentic AI" category). Translations `-es/-pt/-jp/-cn` untouched.
  - Verify: `cargo test -p cobolt-ide` (i18n coverage check — no empty/missing
    translations). Covers **AC9**.

- [ ] **T14 — Finalize** (all)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`.
  - Do: feature → minor (`y`) bump + CHANGELOG entry (confirm operator's
    pre-prod fix/`z` convention at commit time). Review for **AC7** (no
    timer/poll/auto-apply path).
  - Verify: `cargo build` (workspace) + `cargo test -p cobolt-forms --features
    render -p cobolt-ide` all green; manual launch runs the full AC checklist
    (AC1–AC20), incl. multi-turn continuity + restart (**AC13**). Do **not**
    commit/push unless the operator asks.

## Done criteria
All acceptance criteria AC1–AC20 in `spec.md` are checked, tests pass, the English
guide is updated (translations untouched), new `Tr` keys exist in all six
languages, and the change is staged as a feature per the operator's rules
(no commit/push unless asked).
