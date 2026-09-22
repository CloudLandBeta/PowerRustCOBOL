# Spec — RAG / Transactional Chatbot boilerplate (umbrella)

- **Status:** draft → awaiting operator review
- **Folder:** specs/063-rag-chatbot-boilerplate/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-21

> **This is an umbrella spec.** It states the architecture, the invariants and
> the decisions already settled with the operator. It is **not** directly
> implementable: the work decomposes into **two fixes** (outside this pipeline,
> via `/fix` on the `fixes` branch) and **nine features** — `065`–`070`, `072`,
> `073`, `074` — plus the boilerplate project itself (`071`), each feature
> getting its own spec → plan → tasks. Nothing here is built until those exist.
>
> `064` is deliberately unused: it was to be `.cidx` metadata propagation, which
> the operator classified as a **fix** on 2026-09-21, and fixes do not get spec
> folders. The number is left as a gap rather than closed by renumbering, as
> `008` already is.
>
> The multi-agent orchestration is **not** among them — by decision (R67) it is
> COBOL inside the boilerplate, so it ships with `071`.

## 1. Overview

A **boilerplate example project** from which a developer builds a complete
RAG + *transactional* chatbot, delivered as a single PowerRustCOBOL application
binary.

"RAG" is the familiar half: end-user documents — HR policies, marketing
material, ERP manuals — are chunked, embedded and retrieved to ground the
model's answers. "Transactional" is the half that makes it a COBOL product: the
chatbot also reaches **living data** in the developer's own indexed files
through a real **MCP server**, choosing which file to consult from metadata the
developer wrote on the `.cidx` definition. A question about last quarter's
headcount is answered from the data, not from a document describing the data.

Both halves are served by a **mesh of agents** rather than one model call. Each
`AgentObject` the developer drops on the form is an agent with its own model;
deployed together they elect an orchestrator, split the question into tasks, run
the independent ones concurrently, and let the orchestrator compose the answer
from what came back. A question needing both a policy document and a live
headcount dispatches both at once.

The project exists to be copied. Every decision below favours a developer who
will read it, understand it, and change it over one who wants a black box —
which is why the orchestration itself is COBOL in the project (R67) rather than
behaviour hidden in the runtime.

### What already exists, and what this leans on

The IDE's Knowledge Base machinery in `crates/cobolt-agents` is well factored
and content-agnostic: an `Embedder` trait seam, a redb record of
`path → content + embedding` with staleness tracked by content hash and embedder
stamp, chunk chaining, and a `search` that already returns
`KnowledgeHit { path, score, excerpt }`. Verified 2026-09-21: the four KB
modules (`knowledge_store`, `chunked_knowledge`, `project_knowledge`,
`bert_embedder`) import **nothing** from `rig`, Grace, or the rest of
`cobolt-agents` — only std, serde, redb, and candle/tokenizers in the embedder.

They are therefore liftable. That is the *only* thing this project takes from
the existing KB — see the separation rules in §4.1, which are absolute.

## 2. Goals / Non-goals

**Goals**

- A copyable boilerplate for RAG + transactional chatbots over end-user content.
- One executable. Data lives beside it, created at run time, never shipped.
- Retrieval quality good enough to be honest about: semantic when a model is
  available, lexical when it is not, and the difference visible to the user.
- Reuse the KB **architecture and crates**; share none of the KB **data**.
- A developer-facing demonstration of how to localise their own application.
- Parallel, multi-model answering the developer can **read**: adding an agent is
  dropping a control, and the orchestration behind it is COBOL in the project.

**Non-goals**

- Amazon Bedrock memory. The seam is designed for it; the backend is not built.
- PDF ingestion. The route is undecided — see §7 Q3. (DOCX, PPTX and XLSX **are**
  in scope, via `markdownify`; that half of the question is settled.)
- Sharing, extending or reading Grace's System or Project Knowledge Bases.
- Any authentication, authorisation or multi-tenant model.
- Replacing the IDE's own KB, or changing how Grace uses it.

## 3. User stories

- As a **COBOL developer**, I want a working chatbot project I can copy and
  re-point at my own documents and indexed files, so that I ship an AI
  application without learning the host language.
- As an **end user of the built application**, I want to drop a new policy
  document into a folder and have the assistant know it, so that I do not need
  the developer to rebuild anything.
- As an **end user**, I want answers about live business data, not just about
  documents, so that the assistant is useful for operational questions.
- As an **end user**, I want to browse the original documents behind an answer,
  so that I can verify what I was told.
- As a **developer**, I want the assistant's UI in my users' language, so that
  the application is usable outside English.
- As a **developer**, I want to add capacity by dropping another AgentObject on
  the form, so that scaling the assistant does not mean rewriting it.
- As a **developer**, I want to give each agent a different model, so that I can
  spend a strong model where it matters and a cheap one where it does not.
- As an **end user**, I want a question that needs both a document and live data
  answered in one pass, so that I do not have to ask twice.
- As an **operator**, I want a new version of the application to preserve
  everything my users have accumulated, so that upgrading is never a loss.

## 4. Requirements (EARS)

### 4.1 Store separation — the load-bearing invariant

The IDE ships two Knowledge Bases already: the **System KB**
(`~/PowerRustCOBOL/data/chunked.data`, built from `cobolt-compiler` doc
constants) and the **Project KB** (`<project>/data/<name>-chunked.data` and
`data/project-knowledge.redb`). Both serve Grace and her specialists when
generating COBOL. The application KB serves end users asking about their own
content. They are unrelated, and the code must make that structural rather than
conventional.

- **R1 (ubiquitous):** The application Knowledge Base shall be stored in its own
  file, distinct from every store Grace uses.
- **R2 (ubiquitous):** The application Knowledge Base shall use table names
  distinct from `chunks`, `documents` and `project_documents`, so that a
  mis-pointed path fails loudly rather than merging two corpora.
- **R3 (ubiquitous):** The application's source documents shall live in a
  directory that is not `<root>/Knowledge Base/`, which Grace's
  `chunked_knowledge::tree_documents` already claims.
- **R4 (constraint):** The application shall not read, write, extend or depend
  on the System KB or the Project KB.
- **R5 (constraint):** No Grace artefact — `project-knowledge.redb`,
  `<name>-chunked.data`, `project-knowledge.sqlite`, `grace-conversation.json` —
  shall be present in a delivered application folder.

> R5 is currently **violated by the existing build** and is tracked separately:
> `dist/data/` is a verbatim copy of `<project>/data/`, so a delivery today
> carries ~7.3 MB of Grace's data including the developer's own AI conversation
> history. That is a pre-existing defect, classified as a **fix**, and is not
> this feature's work — but this feature's acceptance depends on it.

### 4.2 The Knowledge Base is a derived cache

- **R6 (ubiquitous):** Every record in the application Knowledge Base shall be
  derivable from a file in the live document folder.
- **R7 (event):** When a document is added, changed or removed in the document
  folder, the system shall bring the Knowledge Base into agreement with it.
- **R8 (event):** When the store's schema version does not match the running
  application's, the system shall rebuild the store from the document folder
  rather than migrate it.
- **R9 (constraint):** The system shall not delete a store it supersedes; it
  shall move it aside, following the existing habit that *"nothing here is ever
  the only copy of anything."*
- **R10 (event):** When a rebuild is requested, the system shall warn the user
  that it takes time before starting, and report progress while it runs.

> R6 is what makes R8 safe. The user's data is the *files*; the store is
> reconstructible, so a schema change can never lose anything. This is the
> assumption `chunked_knowledge::retire_obsolete_store` already documents.

### 4.3 Conversation history is a system of record

- **R11 (ubiquitous):** Conversation history shall be kept in a store separate
  from the Knowledge Base, with its own schema and lifecycle.
- **R12 (constraint):** The system shall not rebuild, truncate or discard
  conversation history in response to a schema change.
- **R13 (event):** When the application starts against a conversation store
  written by an older version, the system shall apply the ordered migrations
  between the two versions, preserving all existing data.
- **R14 (ubiquitous):** Conversation history shall be reached through a backend
  abstraction, selected from a configured location string by its scheme, in the
  manner of `db_runtime::BackendKind::classify`.
- **R15 (ubiquitous):** The abstraction shall be expressed in **conversation**
  terms — append a turn, fetch context for a session, enumerate sessions — and
  not in storage terms, so that a server-managed remote memory can implement it.
- **R16 (optional):** Where a backend cannot offer a capability, the abstraction
  shall provide a default rather than require it, in the manner of
  `IndexedStore::commit` / `unlock`.
- **R17 (constraint):** R13's migration guarantee belongs to the **local**
  backend. The abstraction shall not promise it universally, because a remote
  memory's schema is not ours to migrate.
- **R18 (optional):** Where a cloud backend is not enabled, its dependencies
  shall not enter the build graph.
- **R75 (ubiquitous):** The location string that selects the conversation
  backend shall be a **project** setting, fixed at build time, alongside the
  embedder setting — not an end-user preference.

> R18 has teeth. This workspace deliberately avoids rustls, aws-lc-rs and ring
> because they compile C and need cmake — `cobolt-agents` pins `native-tls` and
> disables `tokenizers`' `esaxx_fast` for exactly that reason. The AWS SDK
> defaults to aws-lc-rs. An ungated Bedrock backend would reintroduce the
> toolchain requirement the project spent effort removing.

### 4.4 Transactional access over MCP

- **R19 (ubiquitous):** The application shall expose a **Model Context Protocol**
  server speaking JSON-RPC 2.0.
- **R20 (ubiquitous):** The MCP server shall expose indexed-file search as a
  tool whose schema is generated from each file's declared metadata.
- **R21 (ubiquitous):** An indexed-file definition shall carry a description of
  the file's **purpose** and a description of **each column**, and both shall be
  readable by the running application.
- **R22 (event):** When the user asks a question, the tool shall search the
  selected indexed files without the user naming a file, table or column.
- **R23 (state):** While a file is unmarked in the application's configuration,
  the system shall not search it or offer it to the model.
- **R24 (ubiquitous):** Indexed files used by the chatbot shall be openable with
  `STORAGE MODE IS MEMORY`.

> R21 is nearly free: `IndexedDefinition.comment` and `IndexedField.comment`
> already exist and the IDE's editor writes them. They are discarded at codegen
> (`cobolt-codegen/src/indexed.rs` writes `comment: String::new()`), so the gap
> is propagation, not modelling.

### 4.5 Delivery and layout

- **R25 (ubiquitous):** A built application shall be a single executable.
- **R26 (event):** When the build produces a delivery folder, it shall create a
  document folder as a sibling of `assets/`.
- **R27 (constraint):** The build shall not populate the document folder, and
  shall not ship Knowledge Base or conversation stores.
- **R28 (event):** When the application starts and a required folder or store is
  absent, it shall create it empty.
- **R29 (constraint):** The application shall not overwrite an existing document
  folder or store on start-up or upgrade.
- **R30 (ubiquitous):** Knowledge Base tables shall live under the delivery
  folder's `data/` directory.
- **R74 (constraint):** The delivery document folder shall have no project-side
  counterpart, and the build's directory-staging list shall not include it. It
  is created empty, never copied from anywhere.

### 4.6 Embedding backend

- **R31 (ubiquitous):** The project shall carry a setting selecting the
  embedding backend, defaulting to the built-in embedder.
- **R32 (optional):** Where the built-in embedder is selected, the application
  shall embed locally, using the semantic model when its weights are cached and
  the deterministic hashing embedder otherwise.
- **R33 (optional):** Where the endpoint embedder is selected, the application
  shall obtain embeddings over HTTP from the configured model endpoint, and
  shall not link a local embedding engine.
- **R34 (event):** When semantic embedding is unavailable, the system shall tell
  the user that retrieval is lexical rather than silently degrading.
- **R35 (constraint):** The system shall not mix vectors from different
  embedders in one index.

> R35 is already handled by the existing record stamp; it is stated so a new
> backend cannot quietly break it. R33 matters for build weight: the endpoint
> path must leave candle and tokenizers out of the graph entirely.

### 4.7 Application interface

- **R36 (ubiquitous):** The application shall present a sidebar offering: home,
  document management, indexed-file selection, prompt management, assistant
  selection, a new conversation, conversation history, and the current language.
- **R37 (ubiquitous):** The sidebar shall render rows built from **run-time
  data**, not only from a static menu definition, so that conversation history
  is part of the navigation.
- **R38 (ubiquitous):** The sidebar shall host controls, so that assistant
  selection and language selection sit in the rail.
- **R39 (constraint):** Sidebar behaviour shall be identical on all four
  surfaces the renderer serves — designer canvas, form preview, Run Form, and
  the running shell's MenuPane.
- **R40 (ubiquitous):** The user shall be able to browse the original documents
  behind the Knowledge Base.
- **R41 (ubiquitous):** Document management shall support a folder tree, folder
  creation and deletion, upload into a selected folder, document removal, and
  moving a document between folders by **drag and drop**.
- **R42 (ubiquitous):** Indexed-file management shall list every available file,
  mark which are consultable, attach descriptive metadata, admit files from
  another project as **External Files**, remove an external file from the list
  without touching the file itself, and choose between reading from disk and
  loading into memory.
- **R43 (ubiquitous):** Prompt management shall keep a version history ordered
  by timestamp descending, show which version is active, and keep the active
  version first.
- **R44 (event):** When the user selects a non-active prompt version, the system
  shall ask for confirmation, then promote it to active, move it to the top and
  update its timestamp.
- **R45 (ubiquitous):** The prompt editor shall be resizable by a user-dragged
  grip.
- **R46 (constraint):** No window in the application shall resize itself.
- **R47 (ubiquitous):** The home view shall report monthly usage — input and
  output tokens, and number of conversations.
- **R76 (event):** When a model response arrives, the application shall record
  the token counts it reports. Usage shall be taken from the response itself,
  never read back from a provider API.
- **R48 (ubiquitous):** The application's interface shall be available in the
  six supported languages.
- **R49 (constraint):** The application shall not depend on the IDE's `Tr`
  table, which is not available to a compiled binary. Localisation shall be
  implemented in COBOL, within the project.
- **R50 (constraint):** Language flags shall be images. egui does not ligate
  regional-indicator pairs — `🇧🇷` renders as `B R`, which is why the IDE paints
  its own.

> R45 and R46 are not in tension: a window that never resizes *itself* is the
> rule; a grip the user drags is the sanctioned exception, and `Splitter`
> already provides it.

### 4.8 The agent mesh

Each `AgentObject` dropped on a form is one agent with its own model, endpoint
and prompt. Deployed alone it answers by itself; deployed in numbers the agents
elect an orchestrator and divide the work. The orchestration logic is **COBOL
inside the boilerplate** — a reusable framework the developer reads, copies and
changes — not behaviour hidden in the runtime.

- **R51 (ubiquitous):** Each `AgentObject` deployed on a form shall constitute
  exactly one agent.
- **R52 (state):** While exactly one agent is deployed, it shall be the
  orchestrator and shall itself retrieve, process and answer.
- **R53 (event):** When more than one agent is deployed, the agents shall elect
  an orchestrator before the first question is answered.
- **R54 (ubiquitous):** Election shall favour the agent whose model is best
  suited to orchestration.
- **R55 (optional):** Where an agent's model cannot consume tools, that agent
  shall be elected orchestrator, and tool work shall fall to a tool-capable
  agent.
- **R56 (optional):** Where the best-orchestrator model is also the only
  tool-capable model, **tool capability shall take precedence**: that agent
  shall perform the tool work, and orchestration shall fall to another agent.
- **R57 (optional):** Where every agent uses the same model, the orchestrator
  shall be chosen at random.
- **R58 (event):** When a question arrives, the orchestrator shall decompose it
  into tasks and distribute them among the available agents.
- **R59 (ubiquitous):** Tasks that do not depend on one another shall run
  concurrently.
- **R60 (event):** When every dispatched task has reported, the orchestrator
  shall analyse the collected results before answering the user.
- **R61 (ubiquitous):** Agents shall exchange task assignments and results with
  the orchestrator, so that no agent answers the user directly.
- **R62 (ubiquitous):** Each agent shall carry its own model, endpoint, API and
  prompt.
- **R63 (ubiquitous):** The application system prompt — the active version from
  prompt management (R43) — shall be supplied **only** to the orchestrator.
- **R64 (state):** While an agent is not the orchestrator, its own prompt
  property shall serve as its role prompt.
- **R65 (event):** When two or more deployed agents' models cannot consume
  tools, the IDE shall warn the developer at design time.
- **R66 (state):** While a form is deployed with fewer tool-capable agents than
  it needs, the application shall run **without tool calling** rather than fail.
- **R67 (ubiquitous):** Election and orchestration shall be implemented in COBOL
  within the boilerplate, as a framework the developer can read and modify.
- **R68 (constraint):** The application shall not depend on
  `cobolt-ide/src/model_policy.rs`, which a compiled binary cannot reach.
- **R69 (ubiquitous):** An `AgentObject` shall be able to consume tools.
- **R70 (state):** While no agent's model has changed, the elected orchestrator
  shall remain in office for the life of the session — the election is not
  re-run per question.
- **R71 (event):** When an agent's model changes, the agents shall hold a new
  election.
- **R72 (ubiquitous):** The model capability table the election reads — which
  models can call tools, and how well each orchestrates — shall live in the
  project and be maintained by the developer. It shall not be generated from,
  or validated against, `model_policy.rs`.
- **R73 (state):** While a model is absent from the capability table, it shall
  be treated as not tool-capable.

> **R69 is the prerequisite for all of the above and does not exist today.**
> Verified 2026-09-21: `agent_runtime::AskRequest` carries no tool field,
> `body_for` builds a plain chat body and `parse_reply` returns text. All three
> supported protocols (`OpenAiChat`, `OllamaChat`, `Anthropic`) offer tool
> calling in their real APIs; none is wired for it.
>
> The **concurrency in R59 is already solved**: spec 032's `async_op.rs` runs an
> `Ask` on a background thread, delivers `AsyncOutcome::AgentReply` over the
> interpreter's channel, and guards against late replies by per-control
> generation. N agents can already have N requests in flight.
>
> One fact for whoever implements R55/R56 and `072`: the project already has a
> fallback for models that reject *native* function tools. `model_policy.rs`
> records the live cases — gemma via Ollama Cloud *"returns empty when native
> tool definitions accompany fenced-protocol"* — and answers them not by
> excluding the model but by switching to a fenced text protocol that *"keeps
> every host tool reachable as text."* A model can therefore be tool-capable
> without supporting native function-calling.

## 5. Acceptance criteria

- [x] **AC1** — A built application's delivery folder contains no Grace store
      and no `grace-conversation.json`. *(R5)*
- [ ] **AC2** — The application Knowledge Base file name and table names differ
      from every Grace store's. *(R1, R2)*
- [ ] **AC3** — Pointing the application at Grace's store path fails with a
      clear error instead of reading or merging it. *(R2)*
- [ ] **AC4** — Deleting the Knowledge Base store and restarting reproduces an
      equivalent index from the document folder alone. *(R6, R8)*
- [ ] **AC5** — A document dropped into the live folder is answerable without
      restarting the application. *(R7)*
- [ ] **AC6** — A store written by a prior schema version is moved aside, not
      deleted, and rebuilt. *(R8, R9)*
- [ ] **AC7** — Conversation history survives a schema change, verified by
      reading back turns written under the previous version. *(R12, R13)*
- [ ] **AC8** — A build without the cloud-memory feature has no AWS SDK in
      `cargo tree`, and needs no C toolchain. *(R18)*
- [ ] **AC9** — An MCP client can list tools and call indexed-file search over
      JSON-RPC, and the tool schema carries the file's purpose and column
      descriptions. *(R19, R20, R21)*
- [ ] **AC10** — A question naming no file returns rows from the correct indexed
      file, and returns nothing from unmarked files. *(R22, R23)*
- [ ] **AC11** — Upgrading the application over an existing installation leaves
      documents, Knowledge Base content and conversation history intact. *(R29)*
- [ ] **AC12** — A fresh install creates the document folder and stores empty,
      with no content shipped by the build. *(R26, R27, R28)*
- [ ] **AC13** — With the endpoint embedder selected, `cargo tree` shows neither
      candle nor tokenizers. *(R33)*
- [ ] **AC14** — With no semantic model cached, retrieval works and the user is
      told it is lexical. *(R32, R34)*
- [ ] **AC15** — Sidebar rows built from run-time data render identically on all
      four surfaces. *(R37, R39)*
- [ ] **AC16** — A document is moved between folders by drag and drop, and the
      Knowledge Base reflects the move. *(R41, R7)*
- [ ] **AC17** — Promoting a prompt version asks for confirmation, then reorders
      the list and updates the timestamp. *(R43, R44)*
- [ ] **AC18** — The interface renders correctly in all six languages, with no
      string sourced from the IDE's `Tr` table. *(R48, R49)*
- [ ] **AC19** — No window changes size except by a user-dragged grip, verified
      by rendering frames in all six languages. *(R45, R46)*
- [ ] **AC20** — `cargo test -p cobolt-forms --features render` stays green, and
      `engine_reference_form_parity_static_vs_faces` passes. *(R39)*
- [ ] **AC21** — An `AgentObject` calls a tool and acts on its result, over each
      of the three supported protocols. *(R69)*
- [ ] **AC22** — A form with one agent answers a question end to end, that agent
      acting as orchestrator. *(R52)*
- [ ] **AC23** — A form with several agents elects one orchestrator
      deterministically for a given set of models, and the choice is
      reproducible. *(R53, R54)*
- [ ] **AC24** — With one non-tool-capable and one tool-capable agent, the
      non-tool-capable one orchestrates and the other performs tool work.
      *(R55)*
- [ ] **AC25** — Where the strongest orchestrator model is the only tool-capable
      one, it performs tool work and another agent orchestrates. *(R56)*
- [ ] **AC26** — With every agent on the same model, repeated elections do not
      always return the same agent. *(R57)*
- [ ] **AC27** — A question requiring both KB retrieval and indexed-file probing
      dispatches both concurrently, evidenced by overlapping request windows,
      and the orchestrator answers only after both report. *(R59, R60)*
- [ ] **AC28** — Only the orchestrator's request carries the application system
      prompt; a worker's request carries its own role prompt. *(R63, R64)*
- [ ] **AC29** — Deploying two non-tool-capable agents raises a design-time
      warning in the IDE, and building anyway yields an application that answers
      without tool calling. *(R65, R66)*
- [ ] **AC30** — The election and dispatch logic is COBOL in the project, and
      `cargo tree` for the built application shows no dependency carrying model
      policy. *(R67, R68)*
- [ ] **AC31** — The orchestrator is unchanged across consecutive questions in
      one session, and changing an agent's model triggers a fresh election.
      *(R70, R71)*
- [ ] **AC32** — The build creates the delivery document folder empty, the
      staging list is unchanged, and no project directory is copied into it.
      *(R26, R27, R74)*
- [ ] **AC33** — Token counts recorded for a conversation match what the model
      responses reported, and the home view's monthly totals are their sum.
      *(R47, R76)*
- [ ] **AC34** — A `.docx` dropped into the document folder is chunked and
      answerable, and `cargo tree` shows no C-backed compression crate reached
      through `zip`. *(R7, Q3)*

## 6. Constraints & steering check

**i18n (6 languages).** Two distinct obligations, easily confused:

- *IDE* strings added by features `065`–`070` and `072`–`074` — Model Providers
  style settings, any new panel or menu label, and `073`'s non-tool-capable
  agent warning — are `Tr` fields in all six languages, per `tech.md`'s hard
  constraint. Fix **F1** adds none; **F2** may add one if the exclusion is
  reported to the developer.
- *Application* strings belong to the boilerplate and are **COBOL-side**
  (R48/R49). The IDE's `Tr` table is in `cobolt-ide`, which is not in
  `SDK_CRATES`, so a compiled binary cannot reach it.

**Generated-code contract.** Fix **F1** changes what `cobolt-codegen` emits
for a `.cidx` definition. The developer banner and the regenerate-on-
Build/Run/Debug/Check contract are unchanged; generated COBOL stays read-only
and hand-editing it remains out of the question.

**COBOL stays English.** Data-item names, paragraph names and all generated
COBOL remain English in every language, including inside the boilerplate's own
localisation tables. Only the *values* a translation table holds are localised;
the identifiers holding them are not.

**System KB.** Features `066` (SideMenu), `067` (TreeView drag-and-drop) and
`072` (`AgentObject` tool calling) change control behaviour, properties or
methods, so each must update the `cobolt-compiler` property/method/event doc
tables and regenerate `assets/knowledge/chunked.data` in the same change. A red
`prebuilt_chunked_kb_matches_the_published_documentation` is a real failure, not
an expected one; an unchanged `chunked.data` beside a green freshness test means
the wrong file was edited.

**Developer's Guide.** The boilerplate and every developer-observable platform
feature belong in `docs/developers-guide-en.md`. Under GOLDEN RULE #8 an edit to
the English canonical deletes its five translations, and both `docs_embed.rs`
guards go red until the next minor regenerates them. That red is intended.

**Versioning.** Every change bumps `z` in `crates/cobolt-ide/src/version.rs`
with a dated `CHANGELOG.md` entry. Only the operator raises `x` or `y`.

**Fix vs feature.** The spec-folder features (`065`–`073`, and `071`) are
**features**. Two prerequisites are **fixes**, and neither may share a commit or
a branch with feature work:

1. **`.cidx` metadata propagation** — settled as a fix (§7 Q1). No spec folder;
   `/fix` workflow, `fixes` branch, own commit, f=97 if announced.
2. **Grace's artefacts leaking into the delivery folder** (R5). Raised
   separately; also a fix.

Both must land before the features that depend on them — R21 has no meaning
until the first ships, and AC1 cannot pass until the second does.

**Tests.** New COBOL tests under `tests/cobol/**` report quantified,
human-readable results with per-phase timings, per GOLDEN RULE #7. Verify-first:
no measurement is stated that a run did not produce.

**Not driving the application.** Verification is by build and test. The operator
looks at the UI.

## 7. Open questions

- **Q1 — ✅ Resolved (operator, 2026-09-21): `.cidx` metadata propagation is a
  FIX.** The metadata is already modelled and the IDE's editor already writes
  it; codegen discarding it is an incomplete implementation, not an absent
  capability — consistent with "filling an incomplete catalogue is a fix"
  (operator, 2026-08-31).

  **Consequence:** it does not get a spec folder. It rides the `fixes` branch
  through the `/fix` workflow, in its own commit, never mixed with feature work,
  and is announced on f=97 if an announcement is ever requested. See §8.

- **Q2 — ✅ Resolved (operator, 2026-09-21): there is no project-side folder.**
  The build creates `dist/KB/` empty (R26) and the staging loop stays
  `["assets", "data"]`, untouched — `KB` was never in it, so R27 needs no
  exception to the copy rule. The folder belongs to whoever runs the
  application; a developer testing locally is the user at that moment, and
  `dist/` survives rebuilds, so their documents persist. Captured as R74.

- **Q3 — Office and PDF ingestion. ◐ Half-resolved (operator, 2026-09-21).**

  The operator proposed the [`markdownify`](https://crates.io/crates/markdownify)
  crate to convert source documents to markdown, so the existing heading-driven
  `chunk_markdown` keeps working unchanged. Verified against the crate's own
  dependency list at 0.3.8 (MIT, 2026-08-29):

  **✅ Office formats — adopt it.** `markdownify` converts **DOCX, PPTX and
  XLSX** (via `zip`, `quick-xml` and `calamine`), plus CSV, plain text with
  `encoding_rs` charset detection, and ZIP/TAR archives. Pure Rust, no C
  toolchain, and its markdown output feeds the existing chunker with no
  strategy seam required. This removes what was the expensive half of this
  question.

  > ⚠️ **`zip` must keep its C backends off.** It can pull `bzip2` and `zstd`
  > behind features; enabling either reintroduces the C toolchain requirement
  > this workspace has repeatedly paid to avoid.

  **❌ PDF — the crate does not do it.** Its dependencies are `base64`,
  `calamine`, `csv`, `encoding_rs`, `flate2`, `infer`, `lzma-rust2`,
  `quick-xml`, `tar`, `thiserror`, `zip`. No PDF parser of any kind, and 0.3.8
  declares no features, so there is no optional backend either.

  **Still open: how PDF is handled.** Three ways, and it is worth noting that HR
  policy documents are more often PDF than DOCX, so this is the half that
  matters most for the use case:
  1. **`lopdf`, flat chunking** — already vendored in `cobolt-forms` for the
     Viewer, and it decodes content streams. Extracted text has no headings, so
     chunks fall on size boundaries rather than subject boundaries, which is
     exactly the selectivity `chunked_knowledge` was built to avoid.
  2. **`lopdf` plus heading reconstruction** — infer structure from font size
     and weight. Better chunks; genuinely fiddly, and wrong on any document
     whose typography does not follow the convention.
  3. **A dedicated PDF-to-markdown crate**, evaluated on the same terms applied
     to `markdownify`: pure Rust, no C toolchain, license compatible.

  *Recommend 3, falling back to 1.* Whichever is chosen, the ingestion seam
  should take "bytes in, markdown out" so a better PDF converter can replace a
  worse one without touching the chunker.

- **Q4 — ✅ Resolved (operator, 2026-09-21): a project setting, beside
  `[rag] embedder`.** Fixed at build time; the end user does not choose a
  storage backend. Re-pointing a delivered application at cloud memory needs a
  rebuild, which is acceptable while no cloud backend exists to point at — and
  the scheme classifier (R14) means adding a run-time override later is reading
  one more string, not redesigning anything. Captured as R75.

- **Q5 — ✅ Resolved (operator, 2026-09-21): recorded from model responses.**
  Verified 2026-09-21: `agent_runtime::parse_reply` discards usage entirely, but
  the IDE already parses it in `app.rs` with the provider naming variations
  worked out — `input_tokens`/`prompt_tokens`,
  `output_tokens`/`completion_tokens`, `total_tokens`. Feature `072` already
  modifies `parse_reply` for tool calls, so usage is surfaced in the same
  change, reusing that field list rather than rediscovering it. Captured as R76.

  Reading usage back from provider APIs was the alternative and fails outright:
  it needs billing-scoped keys, and does not exist for local Ollama — which is
  `AgentObject`'s default endpoint.

- **Q6 — ✅ Resolved (operator, 2026-09-21): correct `structure.md`.** It
  describes `feat/<slug>` branches merged `--no-ff`, a workflow the remote
  actively rejects — it refuses merge commits on a working branch. CLAUDE.md
  (long-lived `features`/`fixes`, `--ff-only` sync, per-change branches such as
  `fixes-1.70.142` in practice) is authoritative and stays as written.

  **Not this spec's work.** A steering-document correction is a **fix**, so it
  rides the `fixes` branch in its own commit under GOLDEN RULE #5, never mixed
  with `063`. Tracked here only so the decision is not lost.

- **Q7 — Who owns the agent capability table?**

  **What the table is for.** R54–R56 elect an orchestrator by comparing agents
  on two facts about each one's configured model: **can it call tools**, and
  **how well does it orchestrate**. The election is nothing but that comparison
  — without those two facts every election collapses into R57's coin toss. So
  something the running application can read must map a model name to those two
  properties. That mapping is the capability table.

  **✅ Resolved (operator, 2026-09-21): the developer owns it.** The project's
  table is authoritative; `073`'s design-time warning is advisory. A developer
  using a model the IDE has never seen simply edits the table. No generation
  step, no guard test pinning it to `model_policy.rs`, and the two are allowed
  to differ — because the developer is the one who knows what they pointed the
  agent at. Captured as R72/R73.

  This is the most honest of the three options considered (the others were
  generating the table from `model_policy.rs` at build time, or duplicating it
  behind a guard test). It also keeps the boilerplate free of any coupling to an
  IDE-side file, which R68 requires anyway.

  **Residual, stated as an assumption rather than left open:** a model absent
  from the table is treated as **not tool-capable** (R73). That follows R66's
  established principle — degrade rather than fail — and is the conservative
  reading: an agent wrongly assumed capable fails at run time when the tool call
  is rejected, whereas one wrongly assumed incapable merely orchestrates. Worth
  confirming at `071`, but not worth blocking on.

## 8. Decomposition

Each becomes its own spec folder. Ordered by dependency.

**Two fixes come first, outside this pipeline.** Neither gets a spec folder;
both go through `/fix` on the `fixes` branch, in their own commits:

| Fix | What | Blocks |
|---|---|---|
| **F1** | `.cidx` purpose and column metadata reaches the running program | `065`, and R21 |
| **F2** | Grace's artefacts excluded from the delivery folder | AC1, and R5 |

Then the features:

| # | Feature | Depends on | Why separate |
|---|---|---|---|
| `065` | `cobolt-mcp` — JSON-RPC MCP server, tool schema from **F1** | **F1** | New protocol crate, modelled on `cobolt-dap` |
| `066` | SideMenu — run-time rows and hosted controls | — | Touches four surfaces; benefits every app |
| `067` | TreeView drag-and-drop | — | Control capability; benefits every app |
| `068` | `cobolt-kb` carve-out — own tables, schema version, `SDK_CRATES`, `semantic` gate | — | Changes what **every** compiled app can link |
| `069` | `ConversationStore` — pluggable, scheme-classified, local backend with migrations | — | Independent seam; Bedrock lands later behind it |
| `070` | Delivery layout and the `[rag] embedder` setting | `068` | Build and project-model change |
| `072` | `AgentObject` tool calling — `AskRequest`, `body_for`, `parse_reply`, and the call loop, across all three protocols | `065` | Prerequisite for the whole mesh; benefits every app |
| `073` | IDE design-time warning for a non-tool-capable agent set (R65) | `072` | IDE-side; may read `model_policy.rs` directly |
| `074` | Document ingestion — `markdownify` for DOCX/PPTX/XLSX, plus a PDF route (§7 Q3) | `068` | Bytes in, markdown out; feeds the existing chunker |
| `071` | The boilerplate project itself — **including the agent mesh in COBOL** | all | Consumes everything above |

Listed by dependency, not by number: `071` is last because it consumes
everything, so `072` and `073` precede it despite the higher numbers.

Two notes on shape:

- **The agent mesh is not a platform feature.** R67 puts election, task
  distribution and result analysis in COBOL inside the boilerplate, so it lives
  in `071`. Only its prerequisite (`072`) and its design-time warning (`073`)
  are platform work. This is deliberate: the orchestration is the part a
  developer most needs to read and change, and hiding it in the runtime would
  make the boilerplate a black box.
- **`066` and `067` are independent of everything else** and could ship first.
  They improve every existing application, not only this one.

## 9. Provenance

Every constraint here was stated by the operator in session on **2026-09-21**,
and every claim about existing code was verified against the tree on the same
date at version 1.70.141. Where this spec asserts that something does not exist
— MCP, an application-level schema version, TreeView drag-and-drop, i18n outside
the IDE — that is a checked absence, not an assumption.
