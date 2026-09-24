# Plan — Spec 068: the Application Knowledge Base (`KnowledgeBase` control)

## Context

Spec 068 (`specs/068-application-knowledge-base/spec.md`, all questions settled) gives every
application built with PowerRustCOBOL its **own** Knowledge Base — KB #3, never Grace's System
or Project KB. A new non-visual control, `KnowledgeBase`, holds **collections** of user
documents with a searchable index; COBOL indexes, refreshes and searches them, and an
`AgentObject` can be handed a collection as a tool. It is the first prerequisite of PowerChat
(071); 074 (document import) plugs into its converter seam next.

Binding decisions (operator, 2026-09-23/24): engine in the **runtime** — a built app links
neither `cobolt-ide` nor `cobolt-agents`; index in **redb 4.3 on disk, multi-process
(`MultiWriter`)**, shareable on a LAN; embedders **lexical / endpoint / opt-in built-in BERT**
(project setting; the only one needing a C compiler); built-in model cached **per application
in `<app>/assets/models/`**, shared by its users; progress via **events**, the app draws its
modal; **content-based refresh**, no file watcher; a mismatched-embedder app searches
lexically and **saves new documents text-only**; **rcrun always includes** the built-in
embedder (Run Form parity).

Approved by the operator on 2026-09-24.

## Verified facts the design rests on

- IDE engine (`crates/cobolt-agents/src/`): `knowledge_store.rs` is std-only (`Embedder`
  trait, `HashingEmbedder`, 384 dims); `chunked_knowledge.rs` has pure `chunk_markdown` :162,
  `section_kind` :138, `split_content` :277 (512-char parts chained by `parent`), `fnv1a`,
  dot-product `search` :647 with lexical fallback :681 and chain reassembly :704,
  `retire_obsolete_store` :326 (`.v{N}-obsolete`). It reads a hidden global embedder
  (`project_knowledge::active_embedder` :434/:652) — the lift passes the embedder explicitly.
  `bert_embedder.rs`: multilingual-e5-small, `pick_device`, download via reqwest (not in the
  runtime).
- redb 4.3: `experimental-multiprocess` = `["std","experimental-api-5"]`, no C;
  `Builder::set_concurrency_mode` db.rs:2160. **`begin_write` waits forever** for another
  process's writer (page_manager.rs:926) — R10's bound must come from us.
- Forms always link HTTP (`runtime_features.rs:145`); runtime HTTP is ureq + native-tls
  (`http_runtime.rs`), AgentObject already sends through it.
- `drain_async_ops` (interpreter.rs:3427) applies **all** results before dispatching events
  (:3524) and removes the pending op on every result.
- Delivery `copy_dir_all` (compiler lib.rs:3720, called :2281) **overwrites `assets/`** — it
  would destroy users' `assets/KB` and `assets/models`.
- `assets::current_base()` is set by rcrun (form_gui.rs:225, project folder) and the built
  binary (lib.rs:3205); child forms share their parent's process. `assets::resolve()` falls
  back to a relative path when missing — do not use it to create folders.

## Design

### 1. New SDK crate `crates/cobolt-kb`
Deps: `redb = "=4.3.0"` (feature `experimental-multiprocess`), `serde`, `bincode`. Feature
`semantic` adds `candle-core/nn/transformers 0.11` + `tokenizers 0.23` (`default-features =
false, features = ["onig"]`). No TLS, no UI. Modules:
- `embed.rs` — `Embedder` trait + `HashingEmbedder` (lifted from `knowledge_store.rs`);
  `EndpointEmbedder` (OpenAI `/embeddings`, Ollama `/api/embed`) over an **injected**
  `trait Transport { post_json(..); get_to_file(url, path, progress, cancel) }`.
- `chunk.rs` — `chunk_markdown`, `section_kind`, `split_content`, `fnv1a` (lifted).
- `store.rs` — per collection `<Location>/<Collection>/{documents/, collection.kbindex,
  collection.kbindex.lock}`; tables `kb_meta` (schema version, embedder stamp, dims),
  `kb_passages`, `kb_sources`; open with `ConcurrencyMode::MultiWriter`; a store lacking
  `kb_meta` or holding `chunks`/`documents`/`project_documents` → "not an application
  Knowledge Base"; schema mismatch → move aside `.v{N}-obsolete` (as `retire_obsolete_store`)
  and rebuild. **Bounded write**: poll `File::try_lock()` on the `.lock` file every 25 ms up
  to `WriteWaitMilliseconds` (default 5000) → `Busy`; only then `begin_write`. Chunk/embed
  outside the transaction; commit ≤ 16 documents per transaction. Readers never lock.
- `search.rs` — scoring + chain reassembly + lexical fallback, embedder passed in; stamp
  mismatch (R26a) → lexical with a reason.
- `refresh.rs` — walk `documents/`, `kb_sources` = path → `{fnv1a64, len, chunk_count,
  stamp}`, index only added/changed/removed; progress callback + cancel flag; unreadable file
  → skipped with reason. A mismatched-stamp writer stores passages **text-only** (no vector);
  a matching app's next refresh fills vectors. Embedder change only on explicit `Reindex`.
- `convert.rs` — `trait Converter { extensions(); to_text(&Path) }`, built-ins md/txt: the
  seam 074 fills (074 R22 puts its converters in their own crate).
- `semantic.rs` (`#[cfg(feature="semantic")]`) — port of `bert_embedder.rs` (`model_dir`,
  `model_is_cached`, `pick_device`, load); download rewritten on `Transport::get_to_file`,
  `*.part` then rename. Cache `<app>/assets/models/multilingual-e5-small/`.
- Register in workspace members and `SDK_CRATES` (compiler lib.rs:2642 → 12 crates); the
  closure guard (:10025) then passes. `cobolt-agents` is **not** changed.

### 2. Runtime (`crates/cobolt-runtime`)
- Features: `kb = ["dep:cobolt-kb"]`, `kb-semantic = ["kb","cobolt-kb/semantic"]`; `kb` in
  defaults, `kb-semantic` not. `cobolt-form-host` passes both through; **cobolt-cli (rcrun)
  enables `kb-semantic`**.
- `http_runtime.rs`: `Transport` impl on ureq (`#[cfg(feature="http")]`).
- New `src/kb_runtime.rs`: per-control state, worker threads, cancel `Arc<AtomicBool>`,
  progress coalesced (≤ 1 per 50 ms + final).
- `async_op.rs`: `AsyncOutcome::{KbProgress, KbIndexed, KbSearchDone, KbBusy, KbError,
  KbToolResult}`. `drain_async_ops`: `KbProgress` keeps the pending op; property values ride a
  per-control `kb_event_payloads: HashMap<String, VecDeque<Vec<(String,String)>>>` applied
  when the event is dispatched (:3524, where `onToolCall` is handled), so each handler sees its
  own values in order. Operation timeout 0 (sweep ignores it).
- `exec_method` (interpreter.rs:13935) + `is_known_method` (:17757): KnowledgeBase methods
  below; keep the sync test (:20432) green.
- `interpreter/agent_loop.rs`: `AllowKnowledgeBase(kb [, collection])` / `DenyKnowledgeBase`
  → `KbToolSet`, merged in `agent_offered_tools` (:91), tool `kb_<control>_<collection>`
  `{query, max_results}`; `tool_loop_advance` (:222) branch before `mcp_tools`: lexical and
  built-in run in place; endpoint → `Waiting::KbSearch`, embed on a worker, resume on
  `KbToolResult`. Result: numbered passages `[n] document: <path> § <heading> (score …)`
  (R33).

### 3. The control — COBOL surface
- **Designer properties:** Location (default `assets/KB`), Collection, Embedder
  (Lexical|Endpoint|Builtin), Configuration, EmbeddingURL, EmbeddingAPI, EmbeddingModel,
  WriteWaitMilliseconds (5000), MaximumResults (5). Key `EmbeddingAPIKey` run-time only,
  seeded like AgentObject's, never in the `.cfrm`.
- **Run-time properties:** Busy, LastError, SearchMode, SearchModeReason, ProgressDocument,
  ProgressCurrent, ProgressTotal, AddedCount, UpdatedCount, RemovedCount, SkippedCount,
  SkippedDocuments, ResultCount, CollectionCount, DocumentCount.
- **Methods — sync:** CreateCollection, RemoveCollection, ListCollections, GetCollection(n),
  ListDocuments, GetDocument(n), GetResultDocument(n), GetResultPassage(n),
  GetResultScore(n). **Async (R31):** AddDocument, UpdateDocument, DeleteDocument, Refresh,
  Reindex, Search, FetchModel; Cancel.
- **Events:** onProgress, onIndexed, onSearchComplete, onBusy, onError.

### 4. Forms model, hosts, build
- `crates/cobolt-forms/src/model.rs`: `ControlType::KnowledgeBase` at every AgentObject site
  (:2447, :2587, :2635, :2688, :2759, :2797, supported_events :3440, `is_non_visual` :3706,
  `Control::new` :5319, `runtime_property_names_for` :1353); canvas badge `paint.rs` (:1900,
  :2890, :12431).
- `crates/cobolt-form-host/src/seeding.rs` (`build_object_seed` :227, key resolution
  :137/:164): seed KnowledgeBase; default Location = `current_base()/assets/KB`, joined
  directly, created if missing (R7) — reaches all three hosts.
- Compiler: `RagConfig { embedder }` (`[rag]` in `<Name>.project.toml`) on `CoboltProject`
  (lib.rs ~:855) + IDE mirror (`project_model.rs:39`); `RuntimeFeatures` gains `kb`
  (`scan_forms`: a form with a KnowledgeBase) and `kb_semantic` (from `embedder =
  "builtin"`), folded at :1917, logged at :1937; `all()` includes `kb` but not
  `kb_semantic`. Delivery (`copy_dir_all` at :2281): **skip `assets/KB` and
  `assets/models`**; seed a design-time collection's `documents/` only where that collection
  is absent at the destination; never copy index files.

### 5. IDE and docs
Toolbox entry + icon (`toolbox.rs:195`, `:1418`); properties section (`properties.rs`, like
:8339); `designer.rs:13298`; autocomplete (`editor.rs:508`); control lists (`agent.rs:530`,
`:1912`, `llm.rs:2528`); codegen WS items + stub paragraphs (codegen `lib.rs:527`, `:1391`);
doc tables (compiler lib.rs :4717, :5481, :5568, :5626, :6645) + regenerate
`assets/knowledge/chunked.data`; `Tr` strings ×6 for any new IDE text; English guide chapter
"The KnowledgeBase control" (collections, embedders, sharing and verified shares,
AgentObject tool, `[rag] embedder`), GOLDEN RULE #8.

## Phases
1. `cobolt-kb` engine + unit tests (SDK_CRATES, closure guard).
2. Runtime features, Transport, `kb_runtime`, async outcomes + payload queue, methods,
   AgentObject tool.
3. Forms model + seeding (all hosts) + rcrun `kb-semantic`.
4. Compiler: `[rag]`, RuntimeFeatures, delivery skip.
5. IDE surface, KB doc tables + `chunked.data`, guide, i18n.
6. Tests, measurements, sweeps; version bump + CHANGELOG per commit (`z` only), `features`
   branch.

## Verification
- **AC1/AC11:** `cargo tree` test on a generated manifest — no `cobolt-ide`/`cobolt-agents`;
  without `kb_semantic` no candle/tokenizers/onig; `semantic`-gated offline query test.
- **AC2, AC3, AC6, AC7:** `cobolt-kb` unit tests — foreign store refused; folders created,
  existing untouched; delete index + refresh gives identical ranked results for 10 queries;
  N-1 schema moved aside and rebuilt.
- **AC4/AC5:** `cobolt-kb/tests/multiprocess.rs` spawns the test binary as 3 children (2
  search + 1 index, then 2 index), `check_integrity()` after; a child holding the lock file
  makes a writer return `Busy`. **SMB share run = manual operator step**; guide names only
  verified shares.
- **AC8–AC10, AC12–AC14:** `crates/cobolt-runtime/tests/test_knowledge_base.rs` through the
  interpreter — only changed docs touched; unreadable doc skipped with reason; dead endpoint →
  Lexical + reason; Reindex changes stamp; mismatched app leaves the index hash unchanged and
  saves text-only; progress strictly ordered, counts in onIndexed.
- **AC13b:** fake Transport serves model files; second instance does not fetch.
- **AC15:** mock model server as in `test_agent_tool_calling.rs:37`; tool result names the
  document.
- **AC16:** parity through run-form, child form and compiled-binary paths.
- **AC17:** every test prints documents/passages indexed, per-phase time, searches/s.
- Regression: `test_indexed_redb` (feature enabled workspace-wide must not change it),
  full sweeps `cobolt-forms --features render`, `cobolt-runtime`, `cobolt-form-host`,
  `cobolt-cli`, `cobolt-compiler`, `cobolt-ide --bin cobolt-ide`, `--no-fail-fast`;
  freshness test for `chunked.data` green.

## Risks
redb multi-process is **experimental** (pin `=4.3.0`, gate on the integrity test); range locks
on SMB/NFS unverified until the operator's share run; candle build weight and C compiler for
`builtin` and for rcrun; read-only install folders (Program Files) block `assets/KB` and the
model cache — reported through LastError.
