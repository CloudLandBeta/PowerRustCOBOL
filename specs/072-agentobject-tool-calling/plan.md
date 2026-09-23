# Plan — `AgentObject` tool calling

- **Status:** approved by the operator for straight-through implementation
  (2026-09-22)
- **Spec:** ./spec.md   **Date:** 2026-09-22

## 1. Approach

### The loop, and where each part runs

```text
 Ask(q) ──▶ worker thread: HTTP round 1 ──▶ AgentReply ──▶ agent_delivered
                                                              │ parse_turn
                         ┌────────────── text ───────────────┤──▶ LastReply, onResponse (R7)
                         │                                    │
                         │                  tool calls ───────┤
                         │   indexed-file tool: run now, on the interpreter
                         │     thread (read-only search, bounded by 065's memory limit)
                         │   COBOL tool: queue onToolCall with ToolCallId/Name/Arguments;
                         │     the NEXT COBOL-WAIT-EVENT means the handler returned →
                         │     take its SetToolResult (or "") (R9, Q3)
                         │   all answered ──▶ worker thread: HTTP round n+1 (R6)
                         └── MaximumToolRounds reached ──▶ onError (R11)
```

- **HTTP stays off the interpreter thread** (R12): each round is one spec-032
  `AsyncOp` on the same per-control generation, so `Cancel` and a late reply
  are handled exactly as today (R13). The `PendingOp` is created once at `Ask`
  and kept for the whole loop, so `TimeoutSeconds` bounds the whole question,
  handler waits included (Q3).
- **Tool execution runs on the interpreter thread.** An indexed-file search
  touches the program's own files and is already bounded (065 R33/R34); a COBOL
  tool *is* the program. The spec's R12 wording ("the whole loop runs off the
  interpreter's thread, except R9") is met for everything slow (the network);
  the search is the one deliberate exception, recorded here.
- **One COBOL tool call at a time.** `onToolCall` carries its call through the
  control's `ToolCallId` / `ToolName` / `ToolArguments` properties, which a
  second queued call would overwrite before the first handler ran. So calls are
  presented in order, and the next is queued only when the previous handler has
  returned — detected at the next `COBOL-WAIT-EVENT` (`next_wait_outcome`),
  which the generated event loop only reaches after the handler's `CALL` ends.
  An unbound `onToolCall` therefore answers `""` on the very next wait.

### The wire formats (`agent_runtime.rs`, pure, fully unit-tested)

A conversation for one `Ask` is kept as protocol-neutral turns and rendered per
protocol on every round, so no protocol's quirks leak into the loop.

| | request `tools` | tool call in reply | result sent back | usage |
|---|---|---|---|---|
| OpenAiChat | `[{type:function,function:{name,description,parameters}}]` | `choices[0].message.tool_calls[{id,function:{name,arguments:"json"}}]` | assistant msg with `tool_calls`, then `{role:tool,tool_call_id,content}` | `usage.prompt_tokens` / `completion_tokens` |
| OllamaChat | same `tools` shape | `message.tool_calls[{function:{name,arguments:{…}}}]` (no id — one is minted) | assistant msg with `tool_calls`, then `{role:tool,content,tool_name}` | `prompt_eval_count` / `eval_count` |
| Anthropic | `[{name,description,input_schema}]` | `content[{type:tool_use,id,name,input}]` | assistant `content` array as received, then user `content:[{type:tool_result,tool_use_id,content}]` | `usage.input_tokens` / `output_tokens` |
| Fenced (any) | tool list + reply format appended to the system prompt | a fenced ```` ```json {"tool_calls":[{"tool":…,"args":{…}}]} ```` block in the text | user turn "Tool results:" + a fenced JSON list | as the protocol |

- **R5, byte-identical when no tools:** an agent offering no tools goes through
  today's `body_for` untouched; the new `body_for_turns` is used only when at
  least one tool is offered. Golden-body tests pin the no-tools bodies.
- **R16:** `ToolProtocol` (`Native` | `Fenced`) decides — never the model name.
- **R17/R18:** usage is read from every response (the field names above plus
  `input_tokens`/`output_tokens` on any protocol) and summed over the loop into
  `LastInputTokens`, `LastOutputTokens`, `LastToolCallCount`.

### The COBOL surface (Q1 — methods only)

- `AGENT::AllowFile(fd-name [, cidx-path])` / `AGENT::DenyFile(fd-name)` — mark
  a file consultable for the whole application (Q2: every agent sees it).
  Description from the `.cidx` (`read_description`), layout from the program's
  own `FD` (`RecordLayout`, `ASSIGN`, `RECORD KEY`) — never the other way round
  (065 R30). With no `cidx-path`, the delivered `indexed/` tree is searched for
  the definition whose file name matches `fd-name`.
- `AddTool(name, description)`, `AddToolParameter(tool, name, description)`,
  `RemoveTool(name)`, `SetToolResult(call-id, text)`.
- Properties: `ToolProtocol` (default `Native`), `MaximumToolRounds` (default
  8), read-only `LastInputTokens`, `LastOutputTokens`, `LastToolCallCount`,
  and during `onToolCall`: `ToolCallId`, `ToolName`, `ToolArguments`.
- Event: `onToolCall`.
- `Verbose` (R19) logs each round's request, reply and tool results.

## 2. Affected crates / files

- `crates/cobolt-runtime/src/agent_runtime.rs` — `Turn`, `ToolSpec`,
  `ToolCall`, `body_for_turns`, `parse_turn` (text | tool calls, + usage),
  fenced protocol helpers.
- `crates/cobolt-runtime/src/interpreter.rs` — per-agent `ToolLoop` state;
  `agent_ask` offers tools; `agent_delivered` runs the loop; the
  `next_wait_outcome` hook; the new methods; `is_known_method`; `AllowFile`
  layout from the FD.
- `crates/cobolt-runtime/src/async_op.rs` — reuse `AgentReply`; no new outcome.
- `crates/cobolt-forms/src/model.rs` — seed `ToolProtocol`,
  `MaximumToolRounds`; declare `onToolCall`; runtime-only properties; codegen
  linkage for `onToolCall` needs none (it reads properties).
- `crates/cobolt-form-host/src/seeding.rs` — nothing expected (the new props
  seed like the others); verified by test.
- `crates/cobolt-ide/src/panels/properties.rs` — `ToolProtocol` and
  `MaximumToolRounds` rows (identifiers, not translated).
- `crates/cobolt-compiler/src/lib.rs` — KB: properties, methods, `onToolCall`;
  regenerate `chunked.data`.
- `docs/developers-guide-en.md` — AgentObject: tool calling, with a worked
  COBOL example; MCP section cross-link.

## 3. Data / model changes

- New seeded AgentObject properties: `ToolProtocol = "Native"`,
  `MaximumToolRounds = 8` — backfilled on load for older `.cfrm`s.
- New run-time-only properties (listed in `runtime_property_names_for`):
  `LastInputTokens`, `LastOutputTokens`, `LastToolCallCount`, `ToolCallId`,
  `ToolName`, `ToolArguments`.
- No `.cfrm`, `.cidx` or manifest format change.

## 4. Key decisions & alternatives

- **Neutral turns, rendered per protocol** — Why: the loop is written once; a
  protocol is four pure functions with golden tests. Rejected: storing each
  protocol's raw message JSON in the loop (three copies of the loop logic).
- **One COBOL call at a time, resumed at the next wait** — Why: the tool's data
  rides in properties, and the event loop gives a natural "handler returned"
  point with no new threading. Rejected: a blocking handler call from inside
  the interpreter (re-entrancy into the program from the dispatcher).
- **Indexed search on the interpreter thread** — Why: it needs the program's
  layout and files, is bounded, and the IndexedToolSet is not `Send`-shared
  today. Rejected: moving it to the worker (would need the tool set cloned into
  every round).
- **No-tools path untouched** — the cheapest way to make R5 provable.

## 5. Risks & mitigations

- **Provider formats drift** → every format is a pure function with golden
  tests built from each provider's published shape; a local test server speaks
  all three for the end-to-end tests (AC1).
- **A model that never stops calling tools** → `MaximumToolRounds` (R11, AC4).
- **Bad JSON arguments** → an error *result* to the model, never a failed
  question (R10, AC5).
- **Verbose logs keys** → unchanged behaviour, already documented and warned.
- **AllowFile cannot find a `.cidx`** → returns `0`, logged; the file is simply
  not offered (065 R31).

## 6. Test strategy

- `agent_runtime` unit tests: golden bodies (no tools = today's bytes, AC3);
  tools bodies per protocol; parse of text / tool calls / usage per protocol
  (AC7); fenced parse and render.
- `interpreter` end-to-end against an in-test HTTP server (std `TcpListener`,
  scripted replies) for each protocol: tool call → search → final text →
  `onResponse` (AC1); COBOL tool via `onToolCall` + `SetToolResult` (AC2);
  endless tool calls → `onError` (AC4); unknown tool / bad args (AC5); fenced
  against a server that rejects `tools` (AC6); `Cancel` mid-round (AC8).
  Each reports rounds, calls and elapsed time.
- `every_dispatched_control_method_is_spellable_inline`; KB freshness.
- AC9 (built binary): the interpreter is the same code in all three hosts;
  asserted by the existing parity rule — no host-specific code is added.

## 7. Steering compliance

- [ ] i18n: no new IDE strings expected (property names are identifiers).
- [ ] Generated code untouched (the handler reads properties).
- [ ] English Guide updated.
- [ ] System KB + `chunked.data`.
- [ ] Feature → `features`, `z` bump, CHANGELOG.
- [ ] No model-name heuristics (063 R68, 072 R16).
