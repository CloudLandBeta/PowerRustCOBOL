<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Spec 040 — Model Leaderboard

**Status:** implemented (1.58.0, 2026-08-03)

## Problem

A COBOL proficiency test produced a report window and an appended line in
`agentic_ai/model-benchmarks.jsonl`. Once the window was closed the result was
effectively gone: nothing compared two models, nothing survived a change of
project, and choosing which model to give the project's work to came down to
remembering a number seen days earlier.

## What it does

Every proficiency run lands on a ranked board, reachable from Project Settings →
AI → **Model Leaderboard**.

### R1 — Four boards

Overall rank, Cloud free models, Cloud paid models, Local models. Tier is
derived from the connection (`Tier::classify`): Ollama and its cloud relay are
Local; a `:free` model suffix is Cloud free; anything else is assumed to bill,
because assuming a paid route is free is the mistake that costs money. A
per-entry override exists for when the guess is wrong.

### R2 — The row

`Rank | Model name | Provider | Overall evaluation ***** | [Details] [Run tests]
[Apply to Grace] [Apply to Specialists]`

Stars are painted geometry, not font glyphs, so a partial rating is exact.

### R3 — Details

A modal carrying the model's whole metric sheet: rank, overall score, the
thirteen capability scores, hallucination count, context window, hardware,
quantization, parameters, price, and the reason the last attempt failed if it
did. A metric the model never returned reads **not collected**, never 0 %.

### R4 — Unrated models

A run that could not start (no key, refused connection, rate limit) is recorded
with the provider's error and no scores. That entry shows no stars, takes no
rank number, sorts below every rated model, and its Details / Apply buttons are
disabled. A score never earned is worse than no score.

### R5 — Applying a model

*Apply to Grace* sets the orchestrator's `model_profile`; *Apply to Specialists*
sets it on every `AgentKind::Specialist`. Both create the project model profile
when that model has never been used in this project, and both clear an explicit
`no_model` marker, since assigning a model is the developer overruling it.

### R5a — Every configured model is listed (1.58.1)

A row exists for every model profile, tested or not. The original build created
a row only on a test result, which made a freshly shipped board empty and
indistinguishable from a broken one. An untested model shows *not tested yet*,
stays unrated (no rank number, sorted below every scored model), and offers Run
tests. `Leaderboard::ensure_models` runs on project open and on panel open, and
reports whether anything changed so the file is written only when it did.

Opening a project also replays `agentic_ai/model-benchmarks.jsonl` into the
board, so proficiency tests run before the board existed are not lost. The
archive is append-ordered and replayed in order, so the newest run of a model
stands; a model already carrying a live result is skipped entirely.

### R6 — Storage is machine-wide

`<data_dir>/cobolt/model_leaderboard.json`, beside `llm_config.json` — a model
tested with one project open ranks when the next is opened. The per-project
`agentic_ai/model-benchmarks.jsonl` archive of full report text is unchanged and
is what *Open full benchmark report* reads.

### R7 — Extended test metrics

The proficiency prompt returns eight further scores — indexed files, code
modification, debugging, refactoring, table-driven design, type inference,
inline `INVOKE`, code explanation — plus `hallucination_count`, a whole number
rather than a percentage.

### R8 — Connection capabilities

Alongside a run, the provider is asked for supported input/output token limits,
and for local weights the parameter count and quantization. Ollama answers
`/api/show`; OpenRouter and HuggingFace's router answer the OpenAI-style
`/models` listing with `context_length`, `top_provider.max_completion_tokens`
and `pricing`. A provider that publishes nothing yields `None`, and the board
shows *unknown* rather than zero. The probe never fails the run.

### R9 — What is deliberately not measured

Latency, output tokens, reliability, determinism and peak memory were specified,
built, and then removed on the operator's instruction (2026-08-03). They measure
the machine and the moment rather than the model's grasp of COBOL, and a board
that mixes the two invites picking a model for being quick at being wrong. The
prompt explicitly instructs the model not to report them.

## Decisions on record

| Question | Decision | Date |
|---|---|---|
| Per-project or machine-wide store | Machine-wide | 2026-08-03 |
| Reliability/determinism from repeat runs | Dropped with R9 | 2026-08-03 |
| Free/paid/local classification | Auto-derived, per-entry override | 2026-08-03 |
| KPI summary squares | Built in the prototype, then removed | 2026-08-03 |

## Design record

`crates/cobolt-ide/examples/leaderboard_prototype.rs` is the standalone window
the layout was settled in, kept as history. It still shows the R9 metrics and is
not a specification of shipped behaviour.

## Implementation

- `crates/cobolt-ide/src/leaderboard.rs` — store, tiering, ranking, tests.
- `crates/cobolt-ide/src/panels/leaderboard_modal.rs` — the panel.
- `crates/cobolt-ide/src/llm.rs` — extended prompt schema, `spawn_probe_capabilities`.
- `crates/cobolt-ide/src/app.rs` — recording, action handling, agent assignment.
