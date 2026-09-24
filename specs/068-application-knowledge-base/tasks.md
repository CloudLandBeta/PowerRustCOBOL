# Tasks — Application Knowledge Base (`KnowledgeBase` control)

- **Status:** done (AC4 SMB run and AC16 full three-host run pending, see spec)
- **Plan:** ./plan.md   **Date:** 2026-09-24

Ordered so the workspace stays green after each task. Each commit bumps `z` and
adds a CHANGELOG entry; `features` branch. Tests print quantified results
(GOLDEN RULE #7).

## Phase 1 — the `cobolt-kb` engine

- [x] **T1 — Crate skeleton, SDK registration** (R4)
  - Files: `crates/cobolt-kb/{Cargo.toml,src/lib.rs}`, workspace `Cargo.toml`,
    `crates/cobolt-compiler/src/lib.rs` (`SDK_CRATES` :2642).
  - Do: new crate (`redb = "=4.3.0"` with `experimental-multiprocess`, serde,
    bincode; feature `semantic` empty for now); add to members and `SDK_CRATES`.
  - Verify: `cargo build -p cobolt-kb`; `cargo test -p cobolt-compiler
    sdk_crate_set_is_closed` green; `cargo test -p cobolt-runtime --test
    test_indexed_redb` unchanged.

- [x] **T2 — Embedders and chunker** (R15, R21)
  - Files: `cobolt-kb/src/{embed.rs,chunk.rs}`.
  - Do: lift `Embedder`, `HashingEmbedder` (from `cobolt-agents/src/knowledge_store.rs`),
    `chunk_markdown`, `section_kind`, `split_content`, `fnv1a` (from
    `chunked_knowledge.rs`); `Transport` trait; `EndpointEmbedder` (OpenAI
    `/embeddings`, Ollama `/api/embed`).
  - Verify: unit tests — chunking by heading, 512-char split chain, hashing
    determinism, endpoint bodies/replies against a fake `Transport`.

- [x] **T3 — Store: tables, schema, move-aside, separation** (R1, R2, R3, R5, R7, R12, R13)
  - Files: `cobolt-kb/src/store.rs`.
  - Do: `<Location>/<Collection>/{documents/, collection.kbindex}`; tables
    `kb_meta`/`kb_passages`/`kb_sources`; `MultiWriter`; foreign-store refusal;
    schema mismatch → `.v{N}-obsolete` + rebuild; create missing folders, never
    overwrite.
  - Verify: unit tests for AC2, AC3, AC7.

- [x] **T4 — Bounded writes and "busy"** (R8, R10, R11)
  - Files: `cobolt-kb/src/store.rs`.
  - Do: `.lock` file polled with `File::try_lock` every 25 ms up to
    `WriteWaitMilliseconds`, then `begin_write`; ≤ 16 documents per commit.
  - Verify: unit test — held lock → `Busy` after the bound (AC5).

- [x] **T5 — Refresh, add/update/delete, converter seam** (R14, R16–R20, R26a)
  - Files: `cobolt-kb/src/{refresh.rs,convert.rs}`.
  - Do: content-hash diff of `documents/`; progress callback + cancel; skip
    unreadable with reason; `Converter` trait with md/txt; mismatched stamp →
    text-only passages.
  - Verify: unit tests — only changed docs touched (AC9), unreadable skipped
    (AC10), mismatch leaves vectors absent and index usable lexically.

- [x] **T6 — Search** (R25, R26, R26a, R33)
  - Files: `cobolt-kb/src/search.rs`.
  - Do: scoring, chain reassembly, lexical fallback with reason, hit =
    document + heading + passage + score; `Reindex` switches the stamp.
  - Verify: unit tests — delete index + refresh gives identical ranks for 10
    queries (AC6); Reindex changes stamp (AC13); mismatch → lexical, index hash
    unchanged (AC13a).

- [x] **T7 — Multi-process test** (R8–R11)
  - Files: `cobolt-kb/tests/multiprocess.rs`.
  - Do: the test binary spawns itself as 3 children (2 search + 1 index, then 2
    index); `check_integrity()` afterwards; quantified timings.
  - Verify: `cargo test -p cobolt-kb --test multiprocess` (AC4 local part).

- [x] **T8 — Built-in semantic embedder** (R22, R23, R23a)
  - Files: `cobolt-kb/Cargo.toml` (`semantic` feature: candle 0.11, tokenizers
    0.23 `onig`), `cobolt-kb/src/semantic.rs`.
  - Do: port `bert_embedder.rs` (model dir, cached check, device, load);
    download on `Transport::get_to_file`, `*.part` then rename; cache under
    `<base>/assets/models/`.
  - Verify: `cargo build -p cobolt-kb --features semantic`; fake-transport test —
    fetched once, second instance does not fetch (AC13b).

## Phase 2 — runtime

- [x] **T9 — Runtime features and HTTP transport** (R4, R24)
  - Files: `crates/cobolt-runtime/{Cargo.toml,src/http_runtime.rs,src/lib.rs}`,
    `crates/cobolt-form-host/Cargo.toml`, `crates/cobolt-cli/Cargo.toml`.
  - Do: `kb`, `kb-semantic` features (`kb` default); ureq `Transport`; rcrun
    enables `kb-semantic`.
  - Verify: `cargo build -p cobolt-runtime`, `--no-default-features`, `-p
    cobolt-cli`; runtime sweep green.

- [x] **T10 — Forms model: `ControlType::KnowledgeBase`** (R27)
  - Files: `crates/cobolt-forms/src/{model.rs,paint.rs}`.
  - Do: every AgentObject site (variant, names, size, events, `is_non_visual`,
    defaults, runtime properties); canvas badge.
  - Verify: `cargo test -p cobolt-forms --features render` green.

- [x] **T11 — Interpreter: methods, async outcomes, event payloads** (R16, R17, R28–R31)
  - Files: `crates/cobolt-runtime/src/{kb_runtime.rs,async_op.rs,interpreter.rs}`.
  - Do: workers + cancel + coalesced progress; `AsyncOutcome::Kb*`;
    `drain_async_ops` keeps the pending op on progress; per-control payload
    queue applied at dispatch; `exec_method` + `is_known_method` arms.
  - Verify: `crates/cobolt-runtime/tests/test_knowledge_base.rs` — AC8, AC9,
    AC10, AC12, AC14; `is_known_method` sync test green.

- [x] **T12 — AgentObject tool** (R32, R33)
  - Files: `crates/cobolt-runtime/src/interpreter/agent_loop.rs`, `interpreter.rs`.
  - Do: `AllowKnowledgeBase` / `DenyKnowledgeBase`, `KbToolSet` in
    `agent_offered_tools`, `tool_loop_advance` branch, endpoint path via
    `Waiting::KbSearch` + `KbToolResult`.
  - Verify: mock model server test (as `test_agent_tool_calling.rs`) — the answer
    names the document (AC15).

- [x] **T13 — Seeding for all hosts** (R5, R7, R34)
  - Files: `crates/cobolt-form-host/src/seeding.rs`.
  - Do: seed KnowledgeBase; default Location `current_base()/assets/KB`;
    endpoint key resolved like AgentObject's.
  - Verify: parity test — same results under run-form, child form, compiled
    binary paths (AC16); `cargo test -p cobolt-form-host`.

## Phase 3 — build

- [x] **T14 — `[rag] embedder`, RuntimeFeatures, delivery** (R4, R22)
  - Files: `crates/cobolt-compiler/src/{lib.rs,runtime_features.rs}`,
    `crates/cobolt-ide/src/project_model.rs`.
  - Do: `RagConfig`; `kb` from `scan_forms`, `kb_semantic` from the setting;
    delivery skips `assets/KB` and `assets/models`, seeds a collection's
    `documents/` only when absent, never copies indexes.
  - Verify: compiler tests — manifest features; `cargo tree` on a generated
    manifest shows no `cobolt-ide`/`cobolt-agents`, and without `kb_semantic` no
    candle/tokenizers (AC1, AC11); a delivery test leaves an existing
    `assets/KB` byte-identical.

## Phase 4 — IDE and docs

- [x] **T15 — IDE surface**
  - Files: `crates/cobolt-ide/src/panels/{toolbox.rs,properties.rs,designer.rs,editor.rs}`,
    `agent.rs`, `llm.rs`, `crates/cobolt-codegen/src/lib.rs`, `i18n.rs`.
  - Do: toolbox entry + icon, properties section, autocomplete, control lists,
    codegen WS items + stub paragraphs; any new IDE text as `Tr` ×6.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` (i18n tests included);
    `cargo test -p cobolt-codegen`.

- [x] **T16 — System KB and Developer's Guide**
  - Files: `crates/cobolt-compiler/src/lib.rs` doc tables, `assets/knowledge/chunked.data`,
    `docs/developers-guide-en.md`.
  - Do: document every property, method, event and the tool; regenerate the
    store; guide chapter "The KnowledgeBase control".
  - Verify: `every_control_property_is_documented` and
    `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [x] **T17 — Finalize**
  - Full sweeps `--no-fail-fast`: `cobolt-kb`, `cobolt-forms --features render`,
    `cobolt-runtime`, `cobolt-form-host`, `cobolt-cli`, `cobolt-compiler`,
    `cobolt-codegen`, `cobolt-ide --bin cobolt-ide`; AC17 numbers reported; spec
    ACs checked off; CHANGELOG + `z`.
  - **Operator step:** AC4 on an SMB share (two machines); the guide names only
    verified shares.

## Acceptance-criteria coverage

| AC | Task | AC | Task |
|---|---|---|---|
| AC1 | T14 | AC10 | T5, T11 |
| AC2 | T3 | AC11 | T8, T14 |
| AC3 | T3 | AC12 | T11 |
| AC4 | T7 (+ operator SMB run) | AC13 / AC13a | T6 |
| AC5 | T4 | AC13b | T8 |
| AC6 | T6 | AC14 | T11 |
| AC7 | T3 | AC15 | T12 |
| AC8 | T11 | AC16 | T13 |
| AC9 | T5, T11 | AC17 | every test, T17 |
