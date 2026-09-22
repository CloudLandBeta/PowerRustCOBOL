# Spec — `AgentObject` tool calling

- **Status:** draft → awaiting operator review
- **Folder:** specs/072-agentobject-tool-calling/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-22

> Feature `072` of the umbrella spec **063 — RAG / Transactional Chatbot
> boilerplate** (063 R69, R76, AC21). Depends on `065` (shipped 1.70.148).
> Prerequisite for the agent mesh in `071` and for `073`.

## 1. Overview

An `AgentObject` can ask a model a question. It cannot let the model **use a
tool**. `agent_runtime::AskRequest` has no tools field and no message history,
`body_for` builds a single-turn chat body for each of the three protocols
(`OpenAiChat`, `OllamaChat`, `Anthropic`), and `parse_reply` returns text only —
it discards any tool-call block and the token usage the provider reported.

Meanwhile the tool the model should reach already exists: spec `065`'s
`IndexedToolSet` turns each consultable indexed file into an MCP tool, and
`CALL "COBOL-MCP-SEARCH"` dispatches it in process. What is missing is the loop
between the two: offer the tools with the question, recognise a tool call in the
reply, run it, return the result to the model, and repeat until the model
answers in text.

This feature adds that loop to `AgentObject`, on all three protocols, keeps
`Ask` asynchronous (spec 032), and records the token usage each response
reports (063 R76).

## 2. Goals / Non-goals

**Goals**

- An `AgentObject` offers tools with a question and acts on the model's tool
  calls, over each of the three supported protocols. *(063 AC21)*
- Two tool sources: the application's consultable indexed files (`065`), and
  tools the **COBOL program** answers itself — which is what lets the `071`
  mesh make one agent a tool of another.
- Models without native function-calling can still use tools through a fenced
  text protocol (063's note on R55/R56).
- Token usage from every response is available to the program.
- `Ask` stays non-blocking; `onResponse` still fires once, with the final text.

**Non-goals**

- Choosing *which* agent orchestrates, or electing one — that is `071`.
- The capability table (063 R72) and the IDE warning — `071` / `073`.
- Streaming (`Stream` stays unread, as today).
- Multi-turn conversation memory across separate `Ask` calls — `069` owns
  conversation history. The message list built here lives for one `Ask`.
- Depending on `cobolt-ide`'s `model_policy.rs` (063 R68).

## 3. User stories

- As an **end user**, I want to ask "how many contractors started in Q3" and get
  an answer computed from live data, without naming a file.
- As a **developer**, I want to expose a COBOL paragraph as a tool — "create an
  order", "ask the billing agent" — so the model can act, not only read.
- As a **developer**, I want to know how many tokens each question cost, so the
  application can report monthly usage (063 R47).
- As a **developer** using a local model without function-calling, I want tools
  to still work.

## 4. Requirements (EARS)

### 4.1 Offering tools

- **R1 (ubiquitous):** An `AgentObject` shall be able to offer tools to its model
  with each question.
- **R2 (ubiquitous):** The tools offered shall be (a) the search tool of every
  indexed file the application has marked consultable, and (b) every tool the
  program has declared on that agent.
- **R3 (ubiquitous):** A running program shall be able to mark an indexed file
  consultable, and unmark it, by name. *(065 R16/R32 left the marking to the
  application's own store; nothing outside tests can call it today.)*
- **R4 (ubiquitous):** A running program shall be able to declare a tool on an
  agent with a name, a description, and named parameters each with a
  description, and remove it again.
- **R5 (state):** While an agent offers no tools, the request body shall be
  byte-identical to today's, so existing applications and models are unaffected.

### 4.2 The call loop

- **R6 (event):** When the model's reply contains one or more tool calls, the
  agent shall run each, send every result back to the model in the protocol's
  own format, and ask again.
- **R7 (event):** When the model replies with text and no tool call, the agent
  shall end the loop, set `Result`/`LastReply`, and fire `onResponse` once.
- **R8 (event):** When a call names an indexed-file tool, the agent shall run it
  through the same `065` dispatch as `COBOL-MCP-SEARCH`, read-only.
- **R9 (event):** When a call names a program-declared tool, the agent shall fire
  an event carrying the tool name and its arguments, and continue the loop only
  after the program supplies a result for that call.
- **R10 (event):** When a call names a tool that was not offered, or its
  arguments are not valid JSON, the agent shall return an error result to the
  model rather than fail the question.
- **R11 (constraint):** The loop shall stop after a maximum number of rounds,
  set by a property with a safe default; reaching it shall end the question
  through `onError` with a message saying so.
- **R12 (ubiquitous):** `Ask` shall return immediately, as today; the whole loop
  runs off the interpreter's thread, except the handling of R9, which runs in the
  program's own event handler.
- **R13 (constraint):** `Cancel` and `TimeoutSeconds` shall apply to the whole
  loop, and a reply arriving after either shall be discarded (spec 032's
  per-control generation).

### 4.3 Protocols

- **R14 (ubiquitous):** Native tool calling shall be supported on `OpenAiChat`,
  `OllamaChat` and `Anthropic`, each in its own request and reply format.
- **R15 (optional):** Where the agent is set to the fenced text protocol, tools
  shall be described in the system prompt and calls recognised from a fenced
  JSON block in the reply text, so a model without native function-calling can
  use them.
- **R16 (constraint):** The runtime shall choose between native and fenced from
  the agent's own property, never from a model-name heuristic.

### 4.4 Usage and inspection

- **R17 (event):** When a model response arrives, the agent shall record the
  input and output token counts it reports, recognising the providers' naming
  variants, and accumulate them over the loop. *(063 R76)*
- **R18 (ubiquitous):** After a question, the program shall be able to read the
  totals for that question and the number of tool calls made.
- **R19 (state):** While `Verbose` is on, each round's request, reply and tool
  result shall be logged as the existing single-turn request is — API keys
  included, as today, so the operator's switch keeps its meaning.

## 5. Acceptance criteria

- [ ] **AC1** — Against a local test server speaking each protocol, an agent
      offered one indexed-file tool receives a tool call, runs the search, sends
      the result back, and fires `onResponse` with the model's final text —
      three protocols, three passing tests. *(R6–R8, R14; 063 AC21)*
- [ ] **AC2** — A program-declared tool fires its event with the arguments; the
      handler's result reaches the model in the next round. *(R4, R9)*
- [ ] **AC3** — An agent with no tools sends a body byte-identical to 1.70.149
      for each protocol (golden bodies). *(R5)*
- [ ] **AC4** — A model that calls tools forever stops at the round limit with
      `onError`. *(R11)*
- [ ] **AC5** — Unknown tool and malformed arguments produce an error *result* to
      the model, and the question still completes. *(R10)*
- [ ] **AC6** — With the fenced protocol selected, the same scenario as AC1
      completes against a server that rejects the native `tools` field. *(R15)*
- [ ] **AC7** — Token totals equal the sum of the usage in every scripted reply,
      for each provider's field names. *(R17, R18)*
- [ ] **AC8** — `Cancel` during a tool round discards the late reply; no
      `onResponse` fires. *(R13)*
- [ ] **AC9** — A compiled binary runs AC1's OpenAI-protocol scenario with the
      same events as `rcrun run-form`. *(interpreter-binary parity)*

## 6. Constraints & steering check

- **No new dependency with a C toolchain or TLS stack** — the loop reuses the
  existing HTTP bridge and `cobolt-mcp`'s serde-only types.
- **063 R68:** nothing here reads `cobolt-ide/src/model_policy.rs`.
- **i18n:** no IDE string expected; any added is a `Tr` field in all six
  languages.
- **Generated code:** the program-declared tool event needs handler linkage
  (tool name, arguments) — decided in `/plan`, English identifiers. The existing
  codegen agent stub still assumes a **synchronous** `Ask` (see below).
- **System KB:** new properties, methods and events on `AgentObject` → the
  `cobolt-compiler` doc tables updated and `chunked.data` regenerated in the
  same change. *(063 §6)*
- **Methods must be callable:** every new method name goes into
  `is_known_method`.
- **Three hosts:** `interpreter-binary-parity` applies.
- **Docs:** the Guide's AgentObject and MCP sections gain tool calling;
  `-en.md` only exists today.
- **Fix vs feature:** a **feature** — `features` branch, `z` bump.

### Defects found while surveying — not this feature's work

**Fixes**, for the `fixes` branch, listed so they are not lost:

1. **`SetModel()` has no effect.** It writes `Model`
   (`interpreter.rs` `SETMODEL`); `agent_ask` reads `AgentModel`.
2. `LastReply` has no KB property entry, and the `Verbose` KB text still says a
   form run outside the IDE gets no model.
3. The codegen agent stub (`cobolt-codegen` `write_agent_stubs`) generates a
   synchronous `INVOKE … 'Ask' … RETURNING` and tests `WS-AGENT-ERROR` straight
   after — `Ask` has been asynchronous since 1.65.63, so that code reads an empty
   answer.

## 7. Open questions

- **Q1 — the COBOL surface.** Proposed:
  - Files: `CALL "COBOL-MCP-ALLOW" USING file-name` / `"COBOL-MCP-DENY"`, next
    to the existing `COBOL-MCP-SEARCH`.
  - Agent methods: `AddTool(name, description)`,
    `AddToolParameter(tool, name, description)`, `RemoveTool(name)`,
    `SetToolResult(call-id, text)`.
  - Properties: `ToolProtocol` (`Native` | `Fenced`, default `Native`),
    `MaximumToolRounds` (default 8), read-only `LastInputTokens`,
    `LastOutputTokens`, `LastToolCallCount`, and during the tool event
    `ToolCallId`, `ToolName`, `ToolArguments`.
  - Event: `onToolCall`.
  *Recommendation:* these; the operator may prefer different names.
- **Q2 — are all consultable files offered to every agent?** *Recommendation:*
  yes in `072` — consultability is the end user's decision (065 R32), and
  per-agent restriction belongs to the mesh in `071`.
- **Q3 — waiting for a COBOL tool result.** R9 pauses the loop until the
  program answers. *Recommendation:* the pause counts against `TimeoutSeconds`,
  and a handler that returns without calling `SetToolResult` sends the model an
  empty result rather than hanging.
- **Q4 — fix order.** *Recommendation:* land defect 1 (`SetModel`) on `fixes`
  first — AC tests will set models, and a feature should not be tested through a
  broken setter.
