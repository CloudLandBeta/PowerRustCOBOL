<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — AI Development Agent (dev-time form-building assistant)

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-08

## 1. Approach

Add a **structured-operations mode** to the existing IDE assistant. The developer
types a request in the editor's AI prompt bar (R1, config from `LlmConfig`); the
IDE sends the request plus a **compact form/schema context** to the configured
endpoint (R2, reusing `llm::spawn_request`). The model replies with a **JSON
change-set** (a fenced ```json block) drawn from a fixed vocabulary; the IDE parses
and **validates** it against the real control/property/event schema (R3, R4, R9),
renders a **preview** (R5), and applies it only on **Approve** as a **single
undoable `Cmd::AgentBatch`** (R6). Reject discards it (R7). No path triggers the
agent without a submitted prompt (R8).

The four operations map directly onto the existing model (no new storage formats):

| Op | Applies to | Existing primitive |
|----|-----------|--------------------|
| `deploy_control` | new `Control` in `Form.controls` | `Cmd::AddControl` |
| `set_property` (any key) | `Control` property map **or** field-backed prop | `Cmd::SetProperty` / `set_property` |
| `generate_event_handler` | `EventBinding.code` on a control (or form) event | **new** `Cmd::SetEventCode` |
| `create_procedure` | `Form.user_procedures` (`UserProcedure`) | **new** `Cmd::AddProcedure` |

Validation reuses `model::property_names_for(type)` (canonical settable keys),
`ControlType::from_str`, and `supported_events()` — so the agent's schema can never
drift from what the runtime actually accepts (R9).

### Request composition & conversation memory (R14–R16)

Every request is assembled at **send time** as:

```
[ system  ] AGENT_SYSTEM_PROMPT                 ← R14, every request, never stored
[ system  ] SKILLS (agentic_ai/skills/*, incl.  ← R21, every request, never stored
             rustcobol-extensions)
[ user    ] memory turn 1 (developer request)   ┐
[ assistant] memory turn 1 (agent reply JSON)    │ ← R15, replayed from local memory
[ …                                              ┘
[ user    ] <new developer request>  +  CONTEXT  ← R2, CONTEXT recomputed fresh
```

The **skills** (R21) are RustCOBOL reference text loaded from `agentic_ai/skills/`
— always including `rustcobol-extensions`, which tells the model the handler/
procedure body shape and the `::` property syntax so it never emits plain-COBOL GUI
code. The **memory** stores only the plain conversational turns — the developer's
request text and the agent's reply — in a **local indexed file keyed to the current
form/project** (R15). It deliberately excludes the system prompt, the skills, and
the per-request CONTEXT (all recomputed/reloaded fresh each request, R16), so
history never goes stale or bloats. Memory is loaded on IDE start and appended after
each exchange, so follow-ups like "make it green" resolve against prior turns and
survive restarts.

This mirrors the existing `llm::{load_history, save_history}` keyed-history mechanism
(a per-key local store in the data dir); the dev agent gets its **own** memory key
(namespaced per form/project) so it doesn't mix with the code-assistant history.

## 2. Affected crates / files

- `crates/cobolt-ide/src/agent.rs` — **new** module: change-set types
  (`AgentChangeSet`, `AgentOp`), JSON (de)serialisation, validation against the
  live schema, context builder (form → compact JSON), and the dev-agent system
  prompt. (R2–R4, R9)
- `crates/cobolt-ide/src/llm.rs` — add a request variant that sends the agent
  system prompt + replayed memory + context instead of the code-rewrite prompt
  (reuse `spawn_request` transport; add `compose_agent_message`). The agent request
  always sets `system = AGENT_SYSTEM_PROMPT` and replays memory turns, then appends
  the new request + CONTEXT as the final user turn (R2, R12, R14). Reuse
  `llm::{load_history, save_history}` with a dev-agent-specific key for the local
  memory (R15, R16); the system prompt/CONTEXT are never passed to `save_history`.
- `crates/cobolt-ide/src/panels/editor.rs` — prompt-bar **mode** (Code ↔ Agent);
  on an Agent reply, parse → validate → open the **preview panel** instead of
  writing source; Approve/Reject/Cancel wiring; in-flight + error status. (R1, R5,
  R7, R11, R12)
- `crates/cobolt-ide/src/panels/designer.rs` — new `Cmd` variants
  `AgentBatch(Vec<Cmd>)`, `SetEventCode { control_id, event, old, new }`,
  `AddProcedure { name, code }` (+ their inverses); a public `apply_agent_batch`
  entry so Approve pushes exactly **one** undo step. (R6)
- `crates/cobolt-ide/src/app.rs` — own the transient change-set / preview state at
  the level that already holds the `Form` + designer, and route Approve to the
  designer's undo stack (the agent mutates the same `Form` the designer/editor do).
- `crates/cobolt-ide/src/panels/settings_form.rs` — in the AI section, add an
  **"Edit agent prompt…"** button that opens a **modal editor window** (large
  multiline editor + **Save** / **Reset to default** / **Cancel**). It loads the
  current prompt from `agentic_ai/system-prompt.md` (default text when absent) and
  Save writes it back there — a file read/write via the `agentic_ai` helpers, **not**
  a project-file field. (R17, R18)
- **`agentic_ai/` project folder** (new, in the developer's **project** dir, not
  the repo) — home for all agent-AI resources. This feature writes/reads the prompt
  file here; `agents/` and `skills/` are reserved for later specs. Path helpers +
  read/write live in `agent.rs` (or a small `agentic_ai` module). (R18)
- `crates/cobolt-ide/src/panels/project.rs` — add an **`Agentic AI`** entry to the
  `Category` enum and a `show_category` branch (icon + list of files under
  `agentic_ai/`), plus a `ProjectPanelEvent::OpenAgenticFile(PathBuf)` (and `[+]`
  create). (R20)
- **Project create *and* open paths** (`app.rs` — the create-project action and the
  open-project action) — call one shared **`ensure_agentic_ai_scaffold(project_dir)`**
  helper (in `agent.rs`/`agentic_ai`) that creates `agentic_ai/` and writes any
  **missing** default (`system-prompt.md`, `skills/rustcobol-extensions.md`) — and
  **only** the missing ones, never overwriting an existing file. Defaults are
  `include_str!`-embedded so no external files are needed. Idempotent: safe to call
  every open. (R19)
- `crates/cobolt-ide/src/app.rs` — handle `OpenAgenticFile` by opening the file in
  the IDE editor viewport (text/markdown); saving writes it back. (R20)
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields (×6) for: agent-mode label,
  prompt hint, preview title + op labels (Deploy / Set property / Handler /
  Procedure), before→after, Approve, Reject, "thinking…", error strings, and the
  settings **"Edit agent prompt…"** button + modal title / **Save** / **Reset to
  default** / **Cancel**, and the project-tree **"Agentic AI"** category label.
  (R11, R17, R20)
- `docs/developers-guide-en.md` — new "AI development agent" section (configure,
  prompt, preview/approve, undo). English only. (steering)
- `crates/cobolt-forms/src/model.rs` — **no format change**; may add small helpers
  (e.g. `Control::set_event_code`, `Form::procedure_mut`) if not already present.

## 3. Data / model changes

- **No `.cfrm` schema change.** All ops write existing fields: `Form.controls`,
  control property map / field-backed props, `EventBinding.code`,
  `Form.user_procedures`. Files saved after an approved change-set are ordinary
  `.cfrm` — older IDE builds read them unchanged (backward compatible).
- **New IDE-internal types** (not persisted): `AgentChangeSet { ops: Vec<AgentOp> }`
  and `AgentOp` (one enum arm per operation) in `agent.rs`, plus the new `Cmd`
  arms (in-memory undo only).
- **JSON wire format (Q1 resolved):** the model must return
  `{"operations":[ … ]}` inside a single ```json fence. Each op is
  `{"op":"deploy_control","control_type":"Button","id":"SAVE_BTN","properties":{…}}`,
  `{"op":"set_property","control_id":"…","key":"…","value":"…"}`,
  `{"op":"generate_event_handler","control_id":"…","event":"onClick","code":"…"}`,
  `{"op":"create_procedure","name":"…","code":"…"}`. Values are strings/bools/ints
  matching `PropValue`. Chosen over provider function-calling for **portability**
  across local (Ollama/LMStudio) and cloud endpoints, which the transport already
  targets uniformly.
- **Code storage (Q2 resolved):** handler code = `EventBinding.code` (per control /
  per form event); common procedures = `Form.user_procedures`. Both are already
  woven by `cobolt-codegen` as nested programs and regenerated on
  Build/Run/Debug/Check — so agent-authored code obeys the generated-code contract
  automatically (R10).
- **Conversation memory (R15/R16):** a local file of `ChatTurn`s in the IDE data
  dir, keyed per form/project (e.g. `agent-history-<form-id>`), via the existing
  `llm::{load_history, save_history}`. Stores **only** `user`/`assistant` turns —
  never the system prompt or CONTEXT. Not part of the `.cfrm`; deletable without
  affecting the form.
- **Agent resources folder (R17/R18):** a per-project **`agentic_ai/`** directory
  holds agent-AI resources. This feature uses one file, the editable system prompt
  (e.g. `agentic_ai/system-prompt.md`). **Effective prompt** used by R14 =
  `read(agentic_ai/system-prompt.md).non_empty() else AGENT_SYSTEM_PROMPT`. The
  const is the seed/reset value; Save writes the file, Reset deletes/rewrites it to
  the default. Reserved (future specs, see Q6): `agentic_ai/agents/` (agent
  definitions) and `agentic_ai/skills/` (skills). No `.cfrm`/project-file schema
  change — the folder is plain project files, created on demand; absent folder ⇒
  default behaviour (backward compatible).

  ```
  <project>/
    agentic_ai/
      system-prompt.md               ← editable dev-agent prompt (this feature)
      skills/
        rustcobol-extensions.md      ← default skill (this feature): RustCOBOL vs COBOL-85
      agents/                        ← reserved: agent definitions (future spec)
  ```
  The context builder loads `skills/*.md` and injects them (always
  `rustcobol-extensions`) as system content per request (R21). The default skill
  ships in `specs/025-ai-dev-agent/skills/rustcobol-extensions.md` and is embedded
  in the IDE for seeding.

## 4. Key decisions & alternatives

- **Decision:** JSON change-set in a fenced block, parsed + validated by the IDE.
  **Why:** works on every OpenAI-style endpoint the transport already supports;
  keeps the model output auditable in the preview. **Rejected:** provider
  function/tool-calling (not reliably available on local models); free-form COBOL
  like today's assistant (can't target individual controls/props safely).
- **Decision:** Apply as one `Cmd::AgentBatch` composed of existing + two new leaf
  Cmds. **Why:** satisfies R6 (one Undo reverts everything) and reuses the proven
  add-control / set-property inverses. **Rejected:** whole-`Form` snapshot batch
  (simpler but heavy, and a coarse undo that also captures unrelated edits);
  applying ops directly without undo (violates R6/R7 safety).
- **Decision:** Validate every op against `property_names_for` / `ControlType` /
  `supported_events` before preview; invalid ops shown as errors and non-applyable.
  **Why:** R9, and prevents a hallucinated key/type from corrupting the form.
- **Decision:** Reuse the editor prompt bar with a Code/Agent **mode toggle**
  rather than a new panel. **Why:** the user chose "extend editor prompt bar";
  minimal new surface. **Rejected:** designer chat panel (more UI, deferred).
- **Decision:** Agent is request-only; the send path is the existing manual
  submit. **Why:** R8 — no timers, no on-change hooks, no auto-apply.
- **Decision:** Agent resources live as **files in a per-project `agentic_ai/`
  folder** (the prompt now; agents/skills reserved), edited via a **modal window**
  from Settings, with the const as the seed/reset value. **Why:** R18 — a visible,
  version-controllable, extensible home for agentic assets that also scales to agent
  definitions and skills; per-project satisfies "project settings"; the modal keeps a
  long prompt readable. **Rejected:** a `CoboltProject.agent_system_prompt` field
  (buries the prompt in the project file, doesn't scale to agents/skills); the global
  `LlmConfig` (applies to every project); inline-only editing (poor for a
  multi-paragraph prompt).
- **Decision:** Local conversation **memory** = the existing `llm` keyed-history
  store (a local file keyed/indexed per conversation) under a dev-agent key, holding
  only developer/agent turns. **Why:** R15/R16 with zero new format — the system
  prompt is prepended and CONTEXT recomputed at send time, so neither is ever
  persisted; reusing the proven `load_history`/`save_history` avoids a bespoke store.
  **Rejected:** persisting the fully-composed message (would bake in a stale CONTEXT
  and the prompt); backing memory with the runtime redb indexed-file engine (heavier
  than needed for a short per-form transcript — revisit only if durability/query is
  required).

## 5. Risks & mitigations

- **Risk:** Model returns malformed / non-conforming JSON. → **Mitigation:** strict
  parse; on failure show the raw reply + an error and apply nothing (R12/AC10); the
  system prompt gives an exact schema + one example.
- **Risk:** Hallucinated control ids / property keys / event names. →
  **Mitigation:** validate against live schema; mark invalid ops as errors in the
  preview, blocked from Approve (R9/AC4).
- **Risk:** Large forms blow the token budget in context. → **Mitigation:** send a
  **compact** inventory (id, type, non-default props only) + a shared property/
  event legend rather than the full model; note as a tunable (spec Q4).
- **Risk:** Generated handler/procedure COBOL doesn't compile. → **Mitigation:**
  it's previewed before apply and lives in the normal regenerate/Check loop; the
  agent is a scaffold, not a guaranteed-correct compiler; `rcrun check` still gates.
- **Risk:** Cross-panel state (editor prompt → designer form/undo). → **Mitigation:**
  own the change-set/preview state where the `Form` + designer already live
  (app.rs) and route Approve through the designer's `apply`.
- **Risk:** Undo of `SetEventCode`/`AddProcedure` must restore prior code exactly. →
  **Mitigation:** capture `old` in the Cmd (mirrors `SetProperty`); covered by tests.
- **Risk:** The IDE editor is COBOL-oriented; `agentic_ai/` files are markdown/text
  (R20 "any file editable"). → **Mitigation:** open them in a plain-text editing mode
  (no COBOL codegen/regenerate applied to these files — they are not build inputs);
  reuse the existing editor viewport with syntax features off for non-`.cbl` files.

## 6. Test strategy

- **`agent.rs` unit tests (cobolt-ide):**
  - Parse a well-formed change-set JSON → correct `AgentOp`s (all four kinds).
  - Malformed / unknown-`op` JSON → parse error (nothing applied). (AC10)
  - Validation: unknown control id, unsupported control type, and unknown property
    key each flagged as an error op against a sample `Form`. (AC4, R9)
  - Context builder emits ids/types/non-default props for a sample form.
- **`Cmd` tests (designer):** apply then undo of `AgentBatch` containing
  deploy_control + set_property + set_event_code + add_procedure restores the
  `Form` to its exact prior state (byte-identical serialise); redo re-applies. (AC2,
  AC3, AC5, AC6, AC8, R6)
- **Round-trip:** a form with an agent-added procedure + handler saves/loads as
  normal `.cfrm` (no schema drift).
- **Scaffold test (`ensure_agentic_ai_scaffold`):** on an empty dir it creates
  `agentic_ai/` + `system-prompt.md` + `skills/rustcobol-extensions.md`; run again it
  is a no-op; if `system-prompt.md` was edited it is left untouched while a deleted
  skill file is re-seeded (non-destructive, per-file). Effective-prompt resolver
  returns file contents when present, the default when absent/empty. (AC16, AC17, AC20)
- **Manual/visual (tree):** the "Agentic AI" node appears at the same level as
  Forms/Indexed Files, lists the folder's files, and double-click opens + edits +
  saves one. (AC18)
- **i18n test/check:** every new `Tr` field present in all six language tables
  (extend the existing i18n coverage check). (AC9)
- **Manual/visual:** launch IDE with `LlmConfig` set to a local endpoint; verify
  bar hidden when unconfigured (AC1); "add a Save button" → preview → Approve →
  one control, one Undo removes it (AC2); property edit (AC3); handler + procedure
  gen (AC5/AC6); Reject leaves form unchanged (AC8); cancel in-flight (AC11).
- All new tests **report** human-readable pass/fail counts; verify-first (assert
  only what the run produced).

## 7. Steering compliance

- [x] **i18n:** all new UI strings added as `Tr` fields in all six languages.
- [x] **Generated-code contract:** handler/procedure code stored in
  `EventBinding.code` / `Form.user_procedures`, woven by `cobolt-codegen` with the
  banner and regenerated on Build/Run/Debug/Check; agent never edits the `.cbl`.
- [x] **Docs:** `docs/developers-guide-en.md` updated (English only; translations
  untouched).
- [x] **Fix vs feature:** **feature** → minor (`y`) bump in `version.rs` +
  `CHANGELOG.md`. (Operator's pre-prod convention may version as `z`; confirm at
  commit.)
- [x] **No "cobolt" in user-facing text; COBOL identifiers/source English**
  (agent-generated code constrained to English by the system prompt + validation).

## 8. Resolved spec questions

- **Q1 (wire format):** JSON change-set in a fenced block (see §3). ✅
- **Q2 (code storage):** `EventBinding.code` + `Form.user_procedures` (see §3). ✅
- **Q3 (delete/rename):** out of scope this feature; the `AgentOp` enum is designed
  to accept a future `delete_control` / `rename_control` arm without rework.
- **Q4 (context budget):** compact inventory + shared legend (see §5 risk).
- **Q5 (approve granularity):** whole change-set Approve (chosen); per-op error
  exclusion exists for invalid ops, but healthy ops are approved as a set.
- **Q6 (agentic_ai layout):** this feature places the **prompt** and a **default
  skill** (`skills/rustcobol-extensions.md`) in `agentic_ai/`, and injects skills
  into context (R21). A general **skills discovery/format** beyond "load `*.md` from
  `skills/`" and the **`agents/`** definitions format remain **deferred** to a
  follow-up spec. ✅ (partial — skills now concrete, agents deferred)
