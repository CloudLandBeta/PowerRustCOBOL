# Plan — Spec 071: PowerChat, Phase 1 (vertical slice)

## Context

Spec 071 (`specs/071-powerchat/spec.md`) is the RAG + transactional chatbot example project.
All the runtime prerequisites it lists have shipped: 068 (application KB), 074 (document
import), 075 (registered indexed files), 076 (model list and key store). Only 067 (TreeView
drag and drop, R50) is unbuilt, and R50 is optional.

The operator chose a **vertical slice first** (2026-09-24). Phase 1 is an application a
person can actually use: it opens as a shell, keeps topics, gives each topic documents and a
Knowledge Base, has a settings form for models and keys, and chats with one agent,
remembering conversations and counting tokens. It is English only.

Phase 2 is the agent mesh (R26–R32, R36), the registered-indexed-files view (R20–R25 UI;
the runtime is 075's), prompt versions (R48), the sample-topics installer (Q1), the other
five languages (R44–R46), and document folders as a tree (R49, R50).

## What Phase 1 delivers (requirements covered)

R1–R3, R4–R9, R10–R10f (PowerChat's own files), R11–R19a (through 068/074), R33–R35, R37,
R38–R42, R43 (sidebar entries for the Phase 1 views), R47.

## Verified facts the design rests on

- **Authoring without the IDE.** The handler COBOL lives inside each `.cfrm` (`<Event>` CDATA
  and `<working-storage>` / `<file-control>` / `<file-section>` blocks).
  - `cobolt_codegen::generate(&Form)` produces the form's program. The `.cbl` must exist on
    disk: `rcrun` finds it through `[files] generated`.
  - The main form is recorded twice: `main-form="true"` in the form, and `[forms] main-form`
    with its `main-form-seal` (`main_form_guard::seal`) in the manifest.
  - A SideMenu's `.menu.yaml` carries an HMAC hash (`cobolt_forms::menu::save_menu` writes
    it).
  - All three are produced by a Rust example, `cargo run -p cobolt-ide --example
    powerchat_regen`, so nothing is hand-hashed.
- **Forms.**
  - A form opens another with `INVOKE ME::"OpenFormSync"("ID")` (modal); a child answers with
    `INVOKE SUPER::SetProperty(...)` and closes with `INVOKE ME::Close()`.
  - A SideMenu row action `open-form:X` loads a form into the shell's content pane (the form
    must be Embedded or Both). `onMenuItemClick` sets `SelectedItemId`. Spec 066's run-time
    rows (`AddItem`, `RemoveItem`, `Clear`) list conversations.
- **Chat.**
  - The Viewer's conversation mode (`AppendMarkdown`, `NewConversation`, `JumpToLatest`)
    shows the chat.
  - The `AgentObject` answers through `onResponse` with `LastInputTokens`/`LastOutputTokens`.
    `ModelEntry` (076) selects the model, and `AllowKnowledgeBase(KB, collection)` (068)
    grounds it.
- **Documents.** `FileDropZone` stages files (`CommitFiles()`), and the KnowledgeBase's
  `ImportDocument`, `DeleteDocument`, `ListDocuments`/`GetDocument` and `Refresh` report
  through `onProgress` and `onIndexed`.
- **Relative paths.** ASSIGN paths and the KB `Location` resolve against the project /
  application folder, as in PowerDemo3.

## Design

### Project layout (`examples/PowerChat/`)

- `PowerChat.project.toml`: `[files]` forms, generated and indexed; `[forms] main-form` and
  its seal. No `[ai]` block, no keys (R3).
- `forms/`: `chat-form.cfrm` (main, the shell), `topics-form.cfrm`, `documents-form.cfrm`,
  `settings-form.cfrm`, `home-form.cfrm`, and `Shell-Menu.menu.yaml`.
- `generated/*.cbl`: produced by the regen example.
- `indexed/*.cidx` + `COPYBOOKS/*.FD/.SEL`: PowerChat's own files.
- `data/idxfiles/`: created at first run (`OPEN OUTPUT` when missing, R10e).
- `assets/KB/`: one collection per topic (068).

### PowerChat's own files — `STORAGE MODE IS DISK`, Rust engine, `OPEN I-O`, `COMMIT` per change

| File | Key | Holds |
|---|---|---|
| `TOPICS` | topic id `X(8)` | name, system prompt, created |
| `CONVS` | conversation id `X(12)` (alt key topic, with duplicates) | topic, title, created, input/output tokens |
| `TURNS` | conversation id + sequence `9(5)` | role (`U`/`A`), text `X(1000)` |
| `MODELS` | entry name `X(30)` | API, URL, model (the key lives in the 076 key store) |
| `SETTINGS` | setting name `X(20)` | value `X(200)` — KB location, current topic, current conversation, active model entry |

A file is opened `OPEN I-O`; FILE STATUS 35 (missing) → `OPEN OUTPUT`, `CLOSE`, `OPEN I-O`
(R10e). Each change is followed by `COMMIT` (R10f). Every form declares only the files it
uses, `IS GLOBAL`.

### Forms

- **chat-form (main, shell).**
  - The SideMenu shows Home, Topics, Documents, Settings, New conversation and a
    "Conversations" section, whose run-time rows are the current topic's conversations,
    newest first (R39, R43).
  - The content pane holds the chat: a Viewer (the conversation), a multi-line TextBox and
    a Send button.
  - It carries `AGENT-1` (AgentObject, `ModelEntry` from SETTINGS) and `KB-1`
    (KnowledgeBase, `Location` from SETTINGS, `Collection` = current topic).
  - On load: open the files, hand every MODELS row to the runtime (`COBOL-MODEL-SET`,
    R35/076), and select the topic. With none selected, it opens topics-form (R4).
  - On send: append the user turn, `AllowKnowledgeBase(KB-1, topic)`, and build the prompt
    from the conversation so far, trimmed to a character budget (R41). Then call `Ask`.
  - On response: append it, write both turns, and add the token counts to the conversation
    (R38, R42).
  - A conversation row reloads its turns into the Viewer (R40). Switching topic starts a new
    conversation (R8).
- **topics-form (modal).** Lists the topics, creates one (a name and a system prompt; its
  collection is created with `KB-1::CreateCollection`, R6) and selects one (it writes
  SETTINGS). No topic is special-cased (R10).
- **documents-form (modal).** Lists the current topic's documents (`ListDocuments`), imports
  staged files (FileDropZone → `ImportDocument`) and deletes the selected one.
  - `Refresh()` runs on open, so edits made outside the application are picked up (R16).
  - Progress shows in an overlay panel on the form: a Label and a ProgressBar driven by
    `onProgress`, hidden at `onIndexed` (R16a). Skipped documents are listed with their codes
    (074).
- **settings-form (modal).** Sets the KB location and maintains the model list in a DataGrid:
  add, edit, remove and choose the active entry.
  - Keys are typed into a masked TextBox, stored with `COBOL-KEY-SET` and the field cleared
    at once. The grid shows "key set" or "no key" from `COBOL-KEY-IS-SET`, never the key
    (R33, R34, R37).
  - Each change is written to MODELS/SETTINGS and handed to the runtime with
    `COBOL-MODEL-SET` / `COBOL-MODEL-REMOVE`.
- **home-form (embedded).** The month's input and output tokens and the number of
  conversations, summed from CONVS (R42), plus a "retrieval is lexical" note when the KB
  reports it (R18).

## Tasks

See `tasks.md`.

## Verification

- `crates/cobolt-ide/tests/powerchat_compiles.rs`:
  - every form loads;
  - its generated COBOL equals the committed `generated/*.cbl` (freshness);
  - it parses with no error diagnostics and passes the semantic analyser;
  - the menu loads (hash valid) and the main-form seal matches;
  - no file under `examples/PowerChat/` contains `APIKey`/`Password`/`Token` values (R3).
- `crates/cobolt-runtime/tests/test_powerchat_files.rs` runs PowerChat's file-handling
  paragraphs as a console program against a temporary folder:
  - first-run creation via OPEN OUTPUT, then I-O;
  - topic create / select;
  - conversation and turn write / reload;
  - token sums.
  It reports quantified results (GOLDEN RULE #7).
- **Operator step:** `rcrun run-form examples/PowerChat/forms/chat-form.cfrm` against a real
  model: create a topic, add documents, ask, reopen a conversation. The IDE is never driven
  by the agent.

## Risks

- **Size.** Five forms of hand-written COBOL. The compile test and the file-handling test
  are the safety net; visual layout is checked by the operator.
- **Prompt trimming by characters, not tokens.** It is documented as approximate.
- **One agent in Phase 1.** The mesh (R26–R32) arrives in Phase 2, and the chat form is laid
  out so that more `AgentObject`s can be added without moving anything.

---

# Phase 2

Operator decisions (2026-09-25):
- **three agents** on the chat form, each assigned a model entry in the RAG
  settings (or none);
- **capabilities as fields on each model entry**: "can call tools" and an
  orchestration rank from 1 to 9 (R72; an entry marked neither is not
  tool-capable, R73);
- **order**: mesh → registered files → prompt versions → sample topics →
  languages → documents tree. Each step is committed and usable.

## Step 1 — the agent mesh (R26–R32, R36; 063 R51–R71)

- **MODELS** gains `MDL-TOOLS` (`Y`/`N`) and `MDL-RANK` (`9`). SETTINGS
  holds `AGENT-1-ENTRY` … `AGENT-3-ENTRY`; Phase 1's `MODEL-ENTRY` is read as
  agent 1's.
- **settings-form:** the two new fields. "Use for chat" becomes "Assign to
  agent N" (1–3), plus "Clear agent N".
- **Election (`PC-ELECT`, chat-form),** over the agents that have an entry.
  It is run once per session and again on `onModelChanged` or after settings
  change (R70, R71).
  - One agent: it answers alone (R52).
  - Otherwise the orchestrator is the highest rank. When every agent uses the
    same model it is picked at random (R57). When exactly one agent can call
    tools, that one does the tool work and the next-ranked agent orchestrates
    (R55, R56).
  - The tool worker is the best-ranked tool-capable agent that is not the
    orchestrator.
  - With no tool-capable agent, the chat runs without tools (R66).
  - The orchestrator gets the topic's system prompt (R63). The others keep the
    role prompts designed on them (R64).
- **A question with several agents** runs in three stages, driven by
  `onResponse` (R58–R61):
  1. **PLAN**: the orchestrator is asked to split the question into at most
     three `TASK:` lines.
  2. **WORK**: the tasks are sent concurrently, one Ask per worker. Tool-work
     goes to the tool worker, which holds the KB (and, from step 2, the
     registered files).
  3. **COMPOSE**: once every worker has reported, the orchestrator composes
     the answer from their results.
  Tokens from every request count toward the conversation.
- The status line names the orchestrator and the tool worker.

## Steps 2–6

Planned when each starts, in the order above: registered files (R20–R25),
prompt versions (R48), sample topics (Q1), languages (R44–R46), documents tree
(R49, R50).
