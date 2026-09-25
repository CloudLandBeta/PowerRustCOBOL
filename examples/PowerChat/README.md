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

**The first time**, nothing is set up yet: the chat shows a welcome screen that
says what to do, and the menu stays shut except **RAG settings** (and **Chat**,
the way back). As soon as one agent has a model, the menu opens. Every screen
opens inside the window, in the content pane beside the menu.

1. **RAG settings** — a summary with four buttons; each opens its group in a
   dialog, and the summary says what each group holds:
   - **Knowledge Base folder** — type it, or press **…** to pick it.
   - **Model providers** — add a connection the way the IDE's Model Providers
     Manager does it: a name, the **provider** from the IDE's seventeen
     (OpenAI, Anthropic, Groq, Ollama, …) — the endpoint fills itself in — and
     the **API key** (it goes to the application's key store and is never
     shown again). **Test connection** asks the provider for its models: a
     count, or what to fix, in the IDE's own words.
   - **Model selection** — opening it connects and lists the provider's
     models; pick one, say whether it **calls tools** and its orchestration
     **rank** (1–9).
   - **Agents** — give each agent (1, 2 or 3) a connection, or leave it off.
     With one agent it answers alone; with more, they elect an orchestrator
     that splits each question among the others.

   **Model selection** and **Agents** stay disabled until a provider
   connection exists. Every field explains itself in a tooltip.
   **Export…** writes everything but the keys to a `rag-settings.xml`;
   **Import…** reads one back and names the models that still need a key.
2. **Topics** — create a topic: a name and what the assistant is for (its
   system prompt). Each topic gets its own documents folder and Knowledge Base.
   Or press **Install sample topics** for three ready-made ones (HR, Orders,
   Legal — see `samples/README.md`); **Remove sample topics** takes them out.
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
| `chat-form` (main) | The menu (designed in `forms/SideMenu-1.menu.yaml`, relabelled in the current language, shut until an agent has a model), the welcome screen, the conversation, three agents (`AGENT-1`…`AGENT-3`) with their election and orchestration, and the topic's Knowledge Base (`KB-1`); this month's token totals |
| `topics-form` | Create and open topics; install and remove the sample topics (`samples/`) |
| `documents-form` | The topic's documents as a folder tree: folders, add, delete, refresh |
| `settings-form` | The Knowledge Base folder, the IDE's providers, the model list and its keys, the connection test, XML export and import |
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
