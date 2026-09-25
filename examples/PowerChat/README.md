# PowerChat

A chatbot you copy to build your own: it answers questions about **one topic at
a time**, from that topic's own documents, through a model your users choose.
Written entirely in COBOL forms (spec 071). This is **Phase 1**: topics,
documents, the RAG settings, and a one-agent chat that remembers its
conversations.

## Run it

Open the project in PowerRustCOBOL AI and press **Run**, or:

```bash
rcrun run-form examples/PowerChat/forms/chat-form.cfrm
```

1. **Topics** — create a topic: a name and what the assistant is for (its
   system prompt). Each topic gets its own documents folder and Knowledge Base.
   Or press **Install sample topics** for three ready-made ones (HR, Orders,
   Legal — see `samples/README.md`); **Remove sample topics** takes them out.
2. **RAG settings** — add a model (name, API, endpoint, model, whether it
   calls tools, its orchestration rank 1–9) and its key, and assign it to
   agent 1, 2 or 3. The key goes to the application's key store and is never
   shown again. With one agent it answers alone; with more, they elect an
   orchestrator that splits each question among the others.
3. **Documents** — the topic's documents as a folder tree. Make a folder with
   **New folder** (inside the one selected), select it, and drop Word,
   PowerPoint, Excel, PDF, Markdown or text files on the zone: they land in
   that folder and are indexed, with a progress panel. **Delete** removes a
   document, or a folder once nothing is left in it.
4. **Data files** — register indexed files for the topic by path (local, a
   network path, or `smb://`), each with its `.cidx`. They are only ever read.
5. **Prompt** — keep versions of the topic's system prompt; save a new one or
   bring an older one back (it asks first).
6. **Chat** — ask. Past conversations are listed in the menu; pick one to
   continue it.

The flags at the foot of the menu switch the interface between English,
Portuguese, Spanish, French, Japanese and Chinese, at once.

## What is where

| Form | Does |
|---|---|
| `chat-form` (main) | The menu, the conversation, three agents (`AGENT-1`…`AGENT-3`) with their election and orchestration, and the topic's Knowledge Base (`KB-1`); this month's token totals |
| `topics-form` | Create and open topics; install and remove the sample topics (`samples/`) |
| `documents-form` | The topic's documents as a folder tree: folders, add, delete, refresh |
| `settings-form` | The Knowledge Base folder, the model list and its keys |
| `files-form` | The topic's registered indexed files |
| `prompts-form` | Versions of the topic's system prompt |

PowerChat's own data is seven indexed files in `data/` (see `data/README.md`),
opened `I-O` and committed as each change is made. Every behaviour is data: no
topic is written into the code.

## Editing without the IDE

The COBOL lives inside each `.cfrm`. After editing one by hand, regenerate the
programs, the menu's integrity hash and the main-form seal:

```bash
cargo run -p cobolt-ide --example powerchat_regen
```

`crates/cobolt-ide/tests/powerchat_compiles.rs` fails when they are stale;
`powerchat_runs.rs` plays the application end to end against a scripted model.

## Not yet

Moving a document between folders by dragging it (spec 071 R50) waits for
TreeView drag and drop (spec 067).
