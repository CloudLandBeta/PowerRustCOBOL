# Tasks — `AgentObject` tool calling

- **Plan:** ./plan.md   **Date:** 2026-09-22

- [x] **T1** `agent_tools` (a new module beside `agent_runtime`): `Turn`, `ToolSpec`, `ToolCall`, `Usage`;
      `body_for_turns` for OpenAiChat / OllamaChat / Anthropic; `parse_turn`
      (text | calls, usage). *Verify:* golden tests per protocol; no-tools
      bodies byte-identical to `body_for` (AC3, AC7).
- [x] **T2** fenced protocol: system-prompt tool block, fenced-call parse,
      results turn. *Verify:* unit tests.
- [x] **T3** model: seed `ToolProtocol`, `MaximumToolRounds`; declare
      `onToolCall`; runtime-only properties; backfill. *Verify:* forms tests.
- [x] **T4** interpreter: `AddTool`/`AddToolParameter`/`RemoveTool`/
      `SetToolResult`/`AllowFile`/`DenyFile`; `is_known_method`.
- [x] **T5** interpreter: the loop — offer tools in `agent_ask`, `ToolLoop`
      state, `agent_delivered` dispatch, indexed tools, COBOL tools via
      `onToolCall` + the `next_wait_outcome` hook, round limit, usage totals,
      verbose logging, cancel/timeout.
- [x] **T6** end-to-end tests against a local HTTP server (AC1, AC2, AC4–AC8).
- [x] **T7** properties panel rows; KB + `chunked.data`; Guide.
- [ ] **T8** full sweeps; CHANGELOG + `z` bump.
