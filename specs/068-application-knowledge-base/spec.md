# Spec — Application Knowledge Base (the KnowledgeBase control)

- **Status:** draft → awaiting operator review
- **Folder:** specs/068-application-knowledge-base/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-24
- **Parents:** `specs/063-rag-chatbot-boilerplate/spec.md` (umbrella, §4.1–4.2,
  4.6) and `specs/071-powerchat/spec.md` (§4.3). Where they differ, 071 and the
  operator's answers of 2026-09-24 win.

## 1. Overview

A built application can keep its **own Knowledge Base**: a folder of documents
its users own, and a searchable index derived from them. A COBOL program works
with it through a new non-visual control, **`KnowledgeBase`**, and hands it to an
`AgentObject` as a tool, so a model answers from the users' documents.

This is the third of three Knowledge Bases, and the only one a built
application has:

| KB | Owner | In a built application? |
|---|---|---|
| PowerRustCOBOL (System) KB | Grace and her agents | No |
| Project KB | Grace, at design time, for one project | No |
| **Application KB** (this spec) | The built application's end users | **Yes — the only one** |

It is not derived from, stored with, or shaped by the other two, and shares no
data, file, table or path with them. Its engine — chunking, embedding, search —
may start from the IDE KB's code, but it lives in the **runtime**: a built
application depends on neither the IDE nor `cobolt-agents` (operator,
2026-09-24).

A KB holds one or more **collections**: independent sets of documents, each with
its own index. PowerChat uses one collection per topic; any other application
uses them however it likes.

## 2. Goals / Non-goals

**Goals**

- A generic, reusable KB for any application built with PowerRustCOBOL.
- Several users' applications can read and write one KB at the same time, on
  one machine or on a LAN share.
- Semantic search where a model is available, lexical search otherwise, and
  the user always told which one is running.
- The index is a derived cache: losing it loses nothing.
- Nothing from the IDE side in a built application's build.

**Non-goals**

- Reading DOCX, PPTX, XLSX or PDF. That is spec **074**; this spec reads
  Markdown and plain text, through a seam 074 plugs into (R14).
- An application-wide model list. That is spec **076**; until it lands, the
  control is configured directly (R24).
- Any change to the System KB, the Project KB, or how Grace uses them.
- A progress window drawn by the runtime. The control reports progress; the
  application draws it (R30).
- Watching the file system for changes (R19).
- Authentication or per-user permissions on a shared KB.

## 3. User stories

- As a **COBOL developer**, I drop a `KnowledgeBase` on a form, point it at a
  folder, and give it to an `AgentObject`, so the model answers from my users'
  documents without my writing search code.
- As a **developer**, I choose per application whether semantic search runs
  inside the application or through an embedding model on a server.
- As an **end user**, I add, change or delete a document and the assistant
  knows about it at once, and I watch the progress while it catches up.
- As an **end user on a LAN**, I share one KB with colleagues; what one of us
  adds, all of us can find.
- As an **end user**, I know whether my search is semantic or only lexical.
- As an **operator**, I upgrade the application and nothing my users added is
  lost; an index from an older version is rebuilt, never merged.

## 4. Requirements (EARS)

### 4.1 Separation and placement

- **R1 (ubiquitous):** The application KB shall be stored in files of its own,
  distinct from every store Grace uses (063 R1).
- **R2 (ubiquitous):** Its tables shall have names distinct from `chunks`,
  `documents` and `project_documents`, so that a mis-pointed path fails loudly
  rather than merging two corpora (063 R2).
- **R3 (constraint):** The application KB shall not read, write or depend on the
  System KB or the Project KB (063 R4).
- **R4 (constraint):** A built application's dependency closure shall contain
  neither `cobolt-ide` nor `cobolt-agents`. The engine is a runtime crate among
  the SDK crates; if the IDE later shares its code, the IDE depends on it, never
  the reverse (operator, 2026-09-24).
- **R5 (ubiquitous):** By default a KB shall live under the delivered
  application's `assets/KB` folder, each collection in a folder of its own
  holding its documents and its index.
- **R6 (ubiquitous):** The KB's location shall be settable at run time, and may
  be a folder on another machine in the LAN.
- **R7 (event):** When a KB or collection folder is absent, the control shall
  create it empty rather than fail (063 R28), and shall never overwrite an
  existing one (063 R29).

### 4.2 Shared use

- **R8 (state):** While several processes — on one machine or several — use one
  KB, all of them shall be able to search it and to add, update and remove
  documents at the same time.
- **R9 (constraint):** The index shall be kept in redb 4.3 on disk, opened in
  its multi-process mode (`MultiWriter`, feature `experimental-multiprocess`),
  never the default `ExclusiveWriter`, which locks the whole file (operator,
  2026-09-24).
- **R10 (state):** While another process holds the one write transaction, a
  process wanting to write shall wait a bounded time and then report the KB as
  busy, never fail with a corrupt index.
- **R11 (constraint):** Concurrent use shall never corrupt the index.

### 4.3 A derived cache

- **R12 (ubiquitous):** Every record in the index shall be derivable from a
  document in the collection's folder, so that deleting the index and
  re-indexing reproduces an equivalent one (063 R6).
- **R13 (event):** When the index's schema version does not match the running
  application's, the control shall move the old index aside — never delete it —
  and rebuild from the documents (063 R8, R9).
- **R14 (ubiquitous):** The control shall read Markdown and plain text, and shall
  take any other format through a "document in, text out" seam, which spec 074
  fills for DOCX, PPTX, XLSX and PDF.
- **R15 (ubiquitous):** Documents shall be split into chunks by subject — by
  heading where the text has headings — so that a search returns the passage
  that answers, not the whole file.

### 4.4 Keeping the index current

- **R16 (event):** When a program creates, updates or deletes a document through
  the control, the control shall update the index to match (operator,
  2026-09-24).
- **R17 (event):** When a program asks the control to **refresh** a collection,
  it shall compare the folder against the index — by content, not by date — and
  index only what was added, changed or removed.
- **R18 (ubiquitous):** A refresh shall pick up changes made outside the
  application: in the file manager, or by another user on the LAN.
- **R19 (constraint):** The control shall not rely on an operating-system file
  watcher, which misses events on network shares.
- **R20 (constraint):** A document that cannot be read shall be reported by name
  and skipped; it shall not stop the others from being indexed.

### 4.5 Embedding

- **R21 (ubiquitous):** A KB shall embed with one of three embedders: **lexical**
  (built in, always available), **endpoint** (a model on a server, over HTTP),
  or **semantic built-in** (a model running inside the application).
- **R22 (optional):** Where the developer enables the built-in semantic
  embedder for an application — a **project setting**, because it changes what
  the build links (063 R31's `[rag] embedder`; operator, 2026-09-24) — it shall
  be included in that application's build and run inside it; where they do
  not, it and its dependencies shall be absent from the build.
- **R23 (event):** When the built-in semantic model is enabled but not yet on the
  machine, the control shall fetch it only when the application asks, reporting
  progress — never on its own at start-up.
- **R23a (ubiquitous):** The built-in model (about 470 MB) shall be cached **per
  application**, inside that application's own folder, and shared by every user
  of that installation, so it is fetched once per installation (operator,
  2026-09-24).
- **R24 (ubiquitous):** The endpoint embedder shall be configured on the control
  directly (URL, API, model, key). When spec 076's model list exists, it shall
  also accept an entry from that list.
- **R25 (event):** When the configured embedder cannot be used — no endpoint
  reachable, no built-in model — the control shall search lexically and **say
  so** through a property the program can show (063 R34).
- **R26 (constraint):** One index shall never mix vectors from different
  embedders; changing a collection's embedder re-indexes it (063 R35).
- **R26a (state):** While an application's embedder differs from the one a
  shared collection was indexed with, that application shall search the
  collection lexically and report why, and shall not re-index a collection
  others depend on (operator, 2026-09-24).

### 4.6 The COBOL surface — the `KnowledgeBase` control

- **R27 (ubiquitous):** `KnowledgeBase` shall be a non-visual control in the
  Toolbox, set up in the designer like `IndexedFile` and `AgentObject`.
- **R28 (ubiquitous):** A program shall be able to: choose the location and the
  collection; create, update and delete a document; refresh a collection;
  search it; list its documents; and read, for each search hit, the document,
  the passage and its score.
- **R29 (ubiquitous):** A program shall be able to create and remove
  collections at run time, and list the ones that exist.
- **R30 (event):** While the index is being updated, the control shall raise
  progress events carrying the document in hand and how many of how many,
  and a completion event with how many documents were added, updated, removed
  and skipped. The application draws any progress window from these (operator,
  2026-09-24).
- **R31 (ubiquitous):** Indexing and search shall run without freezing the form:
  the program keeps responding while a large folder is indexed.
- **R32 (ubiquitous):** An `AgentObject` shall be able to be given a
  `KnowledgeBase` collection as a tool, as it is given an indexed file with
  `AllowFile`, and shall then search it by itself when a question needs it.
- **R33 (ubiquitous):** A tool result shall name the document each passage came
  from, so the model can cite it and the program can offer to open it.

### 4.7 The same everywhere

- **R34 (ubiquitous):** The control shall behave identically in `rcrun
  run-form`, in an embedded child form, and in the compiled binary.

## 5. Acceptance criteria

- [ ] **AC1** — `cargo tree` for a built application using `KnowledgeBase` shows
      neither `cobolt-ide` nor `cobolt-agents`. *(R4)*
- [ ] **AC2** — The KB's file and table names differ from every Grace store's;
      pointing the control at a Grace store fails with a clear error. *(R1, R2)*
- [ ] **AC3** — A fresh install creates `assets/KB/<collection>/` empty; an
      existing one is left untouched. *(R5, R7)*
- [ ] **AC4** — Three processes use one KB at once — two searching while one
      indexes, then two indexing together; every search answers, every
      document lands, and redb's integrity check passes. Run on a local disk
      and on an SMB share, and the guide says which shares were verified.
      *(R8–R11)*
- [ ] **AC5** — A writer kept waiting past the bound gets a "busy" result, not an
      error or a corrupt index. *(R10)*
- [ ] **AC6** — Deleting the index and refreshing yields the same search results
      for a fixed set of queries. *(R12)*
- [ ] **AC7** — An index written under an older schema version is moved aside,
      still present, and a new one built. *(R13)*
- [ ] **AC8** — Creating, updating and deleting a document through the control
      each change the next search's results, without a restart. *(R16)*
- [ ] **AC9** — A document added, changed and removed with the file manager is
      picked up by the next refresh, which touches only those three. *(R17,
      R18)*
- [ ] **AC10** — An unreadable document is reported by name, and the rest of the
      folder is indexed. *(R20)*
- [ ] **AC11** — A build without the built-in semantic embedder shows neither
      candle nor tokenizers in `cargo tree` and needs no C compiler; a build
      with it answers semantic queries offline. *(R22)*
- [ ] **AC12** — With no embedder reachable, search still answers, and the
      control reports that it is lexical. *(R25)*
- [ ] **AC13** — Switching a collection's embedder re-indexes it; no query ever
      compares vectors from two embedders. *(R26)*
- [ ] **AC13a** — Two installations share a collection, one on the endpoint
      embedder and one on built-in: the one that did not index it searches
      lexically, reports why, and leaves the index as it was. *(R26a)*
- [ ] **AC13b** — With the built-in embedder, the model is fetched once into the
      application's folder, and a second user of the same installation uses it
      without fetching again. *(R23, R23a)*
- [ ] **AC14** — Progress events arrive in order during indexing, and a
      completion event reports the counts; the form redraws throughout.
      *(R30, R31)*
- [ ] **AC15** — An `AgentObject` given a collection answers a question from one
      of its documents and names that document. *(R32, R33)*
- [ ] **AC16** — The same program gives the same results under `rcrun run-form`,
      as an embedded child form, and as a compiled binary. *(R34)*
- [ ] **AC17** — Tests report quantified results: documents and chunks indexed,
      time per phase, searches per second (GOLDEN RULE #7).

## 6. Constraints & steering check

- **Fix vs feature.** Feature: a new control and a new runtime capability.
  `features` branch.
- **i18n.** The control's Toolbox entry and any IDE text it adds are `Tr` fields
  in all six languages. What the application shows its users (progress, "search
  is lexical") is the application's own text, drawn from the events and
  properties R25/R30 expose.
- **System KB.** A new control, with properties, methods and events: the
  `cobolt-compiler` doc tables are updated and `assets/knowledge/chunked.data`
  regenerated in the same change.
- **Developer's Guide.** A `KnowledgeBase` chapter in the English guide:
  collections, the three embedders, sharing on a LAN and what was verified,
  handing a collection to an `AgentObject`. GOLDEN RULE #8 applies.
- **Toolchain.** The default build of an application stays free of a C
  compiler; only the opt-in built-in embedder may require one (R22).
- **Interpreter–binary parity.** All three hosts (R34); read the
  `interpreter-binary-parity` skill before planning.
- **Generated code.** A `KnowledgeBase` on a form follows the same generated-code
  contract as every non-visual control.

## 7. Open questions

All settled with the operator on 2026-09-24:

- **Q1 — ✅ A project setting** switches the built-in semantic embedder on
  (R22).
- **Q2 — ✅ A per-application cache**, inside the application's folder and
  shared by all its users (R23a).
- **Q3 — ✅ A mismatched application searches lexically** and says why,
  instead of re-indexing a shared collection (R26a).
