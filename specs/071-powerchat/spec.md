# Spec — PowerChat, the RAG + transactional chatbot boilerplate

- **Status:** draft → awaiting operator review
- **Folder:** specs/071-powerchat/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-24
- **Parent:** `specs/063-rag-chatbot-boilerplate/spec.md` (umbrella). Where this
  spec and 063 disagree, this spec wins: it carries the operator's decisions of
  2026-09-23/24, which came after 063.

## 1. Overview

**PowerChat** is an example project, `examples/PowerChat/`, beside PowerDemo3.
A developer copies it to build their own chatbot. The built application is a
single binary, and its end users talk to it about **one topic at a time**.

A **topic** is what the chatbot knows about and can reach: HR, Orders, Legal,
or any topic the end user creates while the application runs. Each topic has:

- its own system prompt;
- its own **documents** and the **application Knowledge Base** built from them;
- its own **indexed files**, which the model queries through tools.

When the application opens, the user picks a topic or creates one. The
application ships with no topic built in (§7 Q1): HR, Orders and Legal are
sample content.

Every question is answered by a **mesh of agents**: each `AgentObject` on the
form is one agent. The agents elect an orchestrator, which splits the question
into tasks, runs the independent ones in parallel, and composes the answer. The
election and orchestration are **COBOL inside the project**, written to be read
and changed (063 R67).

### The three Knowledge Bases — only the third is this spec's

| KB | Owner | Lives | In the built binary? |
|---|---|---|---|
| PowerRustCOBOL (System) KB | Grace and her agents | IDE | No |
| Project KB | Grace, for one project, at design time | `<project>/` | **No** |
| **Application KB** | The built app's end users, at run time | `assets/KB` of the delivered app | Created and kept by the running app |

The application KB is not derived from, stored with, or shaped by the other two.
Its engine — chunking, embedding, search — is **part of the runtime**: a crate
among the SDK crates a built application compiles against, with no dependency
on the IDE or on Grace's `cobolt-agents` (operator, 2026-09-24). The engine code
may be taken from the IDE's KB, but it moves to the runtime side; if the IDE
later shares it, the IDE depends on the runtime crate, never the reverse. It
shares no data, file, table or path with the other two KBs.

## 2. Goals / Non-goals

**Goals**

- A copyable, readable boilerplate for a topic-scoped RAG + transactional
  chatbot, entirely in COBOL forms and handlers.
- Topics are data, not code: adding one needs no rebuild.
- The runtime capabilities it needs are built **generically**, not for PowerChat
  (operator, 2026-09-23). Any application built with PowerRustCOBOL gets them.
- Six languages, chosen by the end user.
- Parallel, multi-model answering the developer can read.

**Non-goals**

- An MCP server reachable from outside the application. 065's transport is not
  built, and this spec does not build it.
- Amazon Bedrock or any remote conversation memory (063 §2).
- Authentication, authorisation, per-user permissions on a shared KB.
- Storing API keys in the OS keychain. **For now** they live in the
  application's settings file, behind a seam that lets the keychain replace it
  later (R34).
- Any change to how Grace uses the System or Project KB.

## 3. User stories

- As an **end user**, I pick the topic I want to talk about when the chatbot
  opens, so that its answers stay on that subject.
- As an **end user**, I create a new topic with its own documents and data
  files without asking the developer, so that the chatbot grows with my work.
- As an **end user**, I register an indexed file by its path — on my disk or on
  a machine in my network — so that the chatbot can answer from live data.
- As an **end user**, I find my earlier conversations in the sidebar after
  restarting the application.
- As an **end user on a LAN**, I share one Knowledge Base with my colleagues,
  so that documents added by one of us are available to all.
- As an **administrator**, I configure where the KB is and which models the
  agents may use in a settings form, without rebuilding the application.
- As a **COBOL developer**, I copy the project, rename the topics and point it at
  my own files, and every piece I need to change is COBOL I can read.
- As a **developer**, I add capacity by dropping another `AgentObject` on the
  form, and give each agent a different model.

## 4. Requirements (EARS)

### 4.1 The project

- **R1 (ubiquitous):** PowerChat shall be an example project in
  `examples/PowerChat/`, laid out like PowerDemo3 (`<Name>.project.toml`,
  `forms/`, `COPYBOOKS/`, `indexed/`, `generated/`, `src/`).
- **R2 (ubiquitous):** Its main form shall carry a SideMenu, so the application
  opens as a shell.
- **R3 (constraint):** No example file shall contain an API key, password or
  token. Model access shall come from the settings form (§4.6).

### 4.2 Topics

- **R4 (event):** When the application opens and no topic is selected, it shall
  present the topics that exist and an option to create a new one.
- **R5 (ubiquitous):** A topic shall have a name, a system prompt, a document
  folder, an application KB, and a list of registered indexed files.
- **R6 (event):** When the user creates a topic, the application shall create
  its document folder and an empty KB, and make it selectable immediately,
  without a rebuild or restart.
- **R7 (state):** While a topic is selected, the agents shall answer only from
  that topic's KB and that topic's indexed files.
- **R8 (event):** When the user switches topic, the application shall start a new
  conversation under the new topic.
- **R9 (ubiquitous):** The list of topics shall be kept by the application in an
  indexed file it owns, and survive restarts.
- **R10 (constraint):** No topic name, file, prompt or behaviour shall be written
  into the COBOL as a special case. HR, Orders and Legal are data.
- **R10a (constraint):** Every indexed file PowerChat owns — topics,
  conversations, prompt versions, token usage — shall use the default Rust
  indexed-file engine (PRCIDXD1), never the redb engine (operator, 2026-09-24).
  This concerns indexed files only; the KB store (R14) is not an indexed file.

### 4.3 The application Knowledge Base (runtime — needs spec 068)

- **R11 (ubiquitous):** Each topic's KB shall live under the delivered
  application's `assets/KB` folder by default, one folder per topic.
- **R12 (ubiquitous):** The KB location shall be a setting (§4.6). It may point
  to a folder on another machine in the LAN.
- **R13 (state):** While several users' applications point at the same KB, all
  of them shall be able to **search** it and **add or remove** documents at the
  same time. Writes are serialised — one write transaction at a time — and a
  waiting writer is told the KB is busy rather than failing.
- **R14 (constraint):** A shared KB shall never be corrupted by concurrent use.
  It shall be opened in redb 4.3's multi-process mode (`MultiWriter`, feature
  `experimental-multiprocess`), not the default `ExclusiveWriter`, which locks
  the whole file (operator, 2026-09-24: redb 4.3 allows several readers and
  writers across processes).
- **R15 (ubiquitous):** The KB shall be derived from the topic's documents, so
  that deleting it and restarting rebuilds an equivalent one (063 R6, R8).
- **R16 (event):** When a document is added to, changed in or removed from a
  topic's document folder, the application shall bring the KB into agreement
  with it (063 R7).
- **R17 (ubiquitous):** The application shall accept Markdown, plain text, DOCX,
  PPTX and XLSX documents (spec 074). PDF is accepted once 074 settles its route.
- **R18 (event):** When semantic embedding is unavailable, the application shall
  say that retrieval is lexical rather than degrade silently (063 R34).
- **R19 (constraint):** The application KB shall not read, write or depend on the
  System KB or the Project KB (063 R4).
- **R19a (constraint):** The KB engine and every other capability PowerChat uses
  shall be in the runtime. A built application's dependency closure shall
  contain neither `cobolt-ide` nor `cobolt-agents` (operator, 2026-09-24).

### 4.4 Registered indexed files (runtime — needs a new spec, §8)

- **R20 (ubiquitous):** A topic's indexed files shall be **registered by path**:
  the user names where the data file and its `.cidx` description are. Nothing
  is copied or attached.
- **R21 (ubiquitous):** A registered path may be local or on a machine in the
  LAN, written in the operating system's own convention: a UNC path
  (`\\server\share\…`) on Windows, the mounted share (`/Volumes/…`) on macOS,
  the mount point (`/mnt/…`, `/media/…`) on Linux.
- **R22 (event):** When a registered path cannot be reached, the application
  shall say which file and why, and answer without it rather than fail.
- **R23 (ubiquitous):** The model shall be able to query a registered file whose
  record layout is not compiled into the program, taking the layout from its
  `.cidx`.
- **R24 (constraint):** A registered file shall be opened **read-only**; the
  chatbot shall never write to the user's data.
- **R25 (constraint):** A file whose `.cidx` describes no purpose or no fields
  shall be refused, with the reason shown (065's rule: the description is what
  makes it queryable).

### 4.5 The agent mesh (COBOL in the project)

Carried from 063 §4.8 unchanged in substance.

- **R26 (ubiquitous):** Each `AgentObject` on the chat form shall be one agent,
  with its own model and role prompt.
- **R27 (state):** While exactly one agent is deployed, it shall orchestrate and
  answer by itself.
- **R28 (event):** When several agents are deployed, they shall elect an
  orchestrator before the first question, favouring the model best suited to
  orchestrate, giving tool work precedence to tool-capable models, and choosing
  at random among equals (063 R53–R57).
- **R29 (event):** When a question arrives, the orchestrator shall split it into
  tasks, run independent tasks concurrently, and answer only after every task
  has reported (063 R58–R61).
- **R30 (ubiquitous):** Only the orchestrator shall receive the topic's system
  prompt; the others use their role prompts (063 R63–R64).
- **R31 (ubiquitous):** The model capability table the election reads shall be
  data in the project, maintained by the developer; a model absent from it is
  treated as not tool-capable (063 R72–R73).
- **R32 (state):** While fewer agents can call tools than the plan needs, the
  application shall answer without tools rather than fail (063 R66).

### 4.6 RAG settings form (a COBOL form in PowerChat)

- **R33 (ubiquitous):** PowerChat shall contain a **RAG settings** form, written
  in COBOL, where the user sets the KB location and maintains a list of models
  (name, API, endpoint, model, key).
- **R34 (ubiquitous):** API keys shall be stored in the application's settings
  file for now, **through a key-store seam** that an OS-keychain store can
  replace without changing the form or the COBOL (operator, 2026-09-24).
- **R35 (ubiquitous):** An `AgentObject` shall take its model **either** from an
  entry in the settings form's model list **or** from its own direct
  configuration (runtime — needs a new spec, §8).
- **R36 (event):** When a model entry an agent uses is changed, the agents shall
  hold a new election (063 R71).
- **R37 (constraint):** The settings form shall never display a stored key in
  clear once saved, nor write it to a log.

### 4.7 Conversations

- **R38 (ubiquitous):** Each conversation shall be kept, turn by turn, in an
  indexed file the application owns, under the topic it belongs to.
- **R39 (ubiquitous):** The sidebar shall list the current topic's conversations
  as run-time rows (066), newest first.
- **R40 (event):** When the user picks a conversation, the application shall
  reload its turns into the chat Viewer and continue it.
- **R41 (ubiquitous):** Because an `AgentObject` remembers nothing between
  questions, the application shall send the conversation so far with each
  question, trimmed to fit the model.
- **R42 (event):** When a model response arrives, the application shall record
  its token counts (063 R76), and the home view shall show the month's input and
  output tokens and number of conversations (063 R47).

### 4.8 Interface and languages

- **R43 (ubiquitous):** The sidebar shall offer: home, topics, documents,
  indexed files, prompt, RAG settings, new conversation, and the conversation
  history.
- **R44 (ubiquitous):** The interface shall be available in the six supported
  languages, from translation tables in COBOL, and shall not depend on the IDE's
  `Tr` table (063 R48, R49).
- **R45 (ubiquitous):** The language picker shall sit in the sidebar's footer
  panel, with flags drawn as images (063 R50).
- **R46 (event):** When the user picks a language, every visible text shall
  change at once, without a restart.
- **R47 (constraint):** No window shall resize itself; editors that need room
  get a user-dragged grip (063 R45, R46).
- **R48 (ubiquitous):** The prompt editor shall keep versions per topic, newest
  first, with the active one marked; promoting an older version asks first
  (063 R43, R44).

### 4.9 Documents

- **R49 (ubiquitous):** The documents view shall show the topic's document
  folder as a tree, and allow creating and deleting folders, adding documents,
  and removing them.
- **R50 (optional):** Where TreeView drag and drop (067) is available, a document
  shall move between folders by dragging it (063 R41).

## 5. Acceptance criteria

- [ ] **AC1** — The project opens in the IDE, builds, and the built binary starts
      in shell mode with the topic picker. *(R1, R2, R4)*
- [ ] **AC2** — `grep -rE "APIKey|Password|Token"` over `examples/PowerChat`
      finds no secret. *(R3)*
- [ ] **AC3** — A topic created in the running application is selectable at once,
      has an empty document folder and KB, and is still there after a restart.
      *(R5, R6, R9)*
- [ ] **AC4** — With topic A selected, a question whose answer is only in topic B's
      documents or files is not answered from them. *(R7)*
- [ ] **AC4a** — Every indexed file PowerChat creates opens as PRCIDXD1; none is
      a redb container, and nothing in the project selects the redb engine.
      *(R10a)*
- [ ] **AC5** — `grep` of the COBOL sources finds no topic name used as a
      condition. *(R10)*
- [ ] **AC6** — A document dropped into a topic's folder becomes answerable
      without a restart; deleting the KB and restarting rebuilds it. *(R15, R16)*
- [ ] **AC6a** — `cargo tree` for a built PowerChat shows neither `cobolt-ide`
      nor `cobolt-agents`. *(R19a)*
- [ ] **AC7** — Three application instances use one KB at once — two searching,
      one adding a document, then two adding at the same time; every search gets
      an answer, every document lands, and redb's integrity check passes
      afterwards. Run locally and on a network share (§7 Q5). *(R13, R14)*
- [ ] **AC8** — An indexed file registered by a local path, and one by a network
      path in the OS's convention, are both queried by the model; an unreachable
      path is reported by name and the answer comes without it. *(R20–R22)*
- [ ] **AC9** — A registered file whose layout is not compiled into the program is
      queried from its `.cidx`, and nothing is ever written to it. *(R23, R24)*
- [ ] **AC10** — A `.cidx` without purpose or field descriptions is refused with
      the reason. *(R25)*
- [ ] **AC11** — The mesh ACs of 063 hold on PowerChat: one agent answers alone;
      several elect reproducibly; independent tasks overlap in time; only the
      orchestrator's request carries the topic prompt. *(R26–R32; 063 AC22–AC28)*
- [ ] **AC12** — A model entered in the settings form is used by an agent that
      selects it; the key is stored through the key-store seam and never shown
      in clear or logged. *(R33–R35, R37)*
- [ ] **AC13** — A conversation survives a restart and continues where it left
      off; its turns reach the model with each new question. *(R38–R41)*
- [ ] **AC14** — Monthly token totals on the home view equal the sum of what the
      responses reported. *(R42)*
- [ ] **AC15** — Switching language changes every visible text in all six
      languages, with no text from the IDE's `Tr`. *(R44–R46)*
- [ ] **AC16** — No window changes size except by a grip, checked by rendering
      frames in all six languages. *(R47)*
- [ ] **AC17** — Promoting an older prompt version asks for confirmation, then
      reorders the list. *(R48)*

## 6. Constraints & steering check

- **i18n.** Two separate obligations. Any **IDE** string a prerequisite spec adds
  is a `Tr` field in all six languages. PowerChat's **own** strings are COBOL
  translation tables; identifiers stay English, only values are translated.
- **Generated code.** Unchanged contract: generated COBOL is regenerated on
  Build/Run/Debug/Check and never hand-edited.
- **System KB.** Every prerequisite that adds a control property, method, event
  or runtime CALL updates the `cobolt-compiler` doc tables and regenerates
  `assets/knowledge/chunked.data` in the same change.
- **Developer's Guide.** PowerChat gets a chapter in the English guide, and each
  runtime prerequisite documents its own surface. GOLDEN RULE #8 applies.
- **Fix vs feature.** PowerChat and every prerequisite below are **features**:
  `features` branch, never mixed with fixes.
- **Generic runtime.** Anything PowerChat needs from the runtime is specified and
  built as a general capability, with no reference to PowerChat.
- **Not driving the application.** Verification is by build and tests; the
  operator looks at the UI.

## 7. Open questions

- **Q1 — Sample content vs "the build ships no KB".** 063 R27 says a build ships
  neither documents nor KB. HR, Orders and Legal need sample documents and sample
  indexed files to be a useful demo. Proposal: the samples live in the project's
  `samples/` folder, and the running application offers **"Install sample
  topics"**, which copies them into the user's KB location on request. Nothing is
  pre-populated.
- **Q2 — Where do per-topic settings live when the KB is shared?** A shared KB
  implies shared topics. Proposal: the topic list and each topic's prompt live
  **with the KB** (so every user on the LAN sees the same topics), while the model
  list and keys stay **per machine** (R34).
- **Q3 — PDF.** Inherited from 063 Q3, to be settled in 074.
- **Q5 — Byte-range locks on a network share.** redb's multi-process modes rely
  on byte-range file locks, which it supports on Linux, macOS and Windows. A KB on
  an SMB or NFS share additionally needs the share to honour them, which redb's
  documentation does not address. 068 must test two machines writing one KB on a
  real share before AC7 counts as met, and say plainly in the guide which shares
  were verified. The feature is also flagged *experimental* by redb, so its API
  may move between releases.
- **Q4 — Network paths that are not mounted.** On macOS and Linux a share must
  be mounted before a path can reach it. Proposal: the application accepts the
  mounted path only, and reports an `smb://` URL as "mount this share first"
  rather than mounting it itself.

## 8. Prerequisites — built first, generically, each with its own spec

| Spec | Capability | Status |
|---|---|---|
| 065 | Indexed-file tool; `.cidx` descriptions reach the model | ✅ shipped 1.70.148 |
| 066 A | SideMenu run-time rows | ✅ shipped 1.70.161 |
| 072 | `AgentObject` tool calling, token counts | ✅ shipped 1.70.162 |
| **068** | Application KB store: `assets/KB`, per topic, shared across processes with redb 4.3 `MultiWriter`; the engine (chunking, embedding, search) as a **runtime** SDK crate with no IDE or `cobolt-agents` dependency; COBOL surface to index and search; lexical fallback | To specify |
| **074** | Document import: DOCX/PPTX/XLSX via `markdownify`; PDF route | To specify |
| **075** | Indexed files registered **by path** (local or network), layout from `.cidx`, read-only | To specify — new |
| **076** | Application model list: `AgentObject` takes a model from a run-time list or its own configuration; key-store seam, settings-file store first | To specify — new |
| 067 | TreeView drag and drop (only R50) | Specified, not built |

Order: 068 → 074 → 075 → 076 → 071. 070 (delivery layout) is folded into 068,
since the operator placed the KB in `assets/KB`.
