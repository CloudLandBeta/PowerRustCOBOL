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
   that folder and are indexed — a bar for the chunking and one for the
   embedding, and a line saying what each file became. A file whose name is
   already in that folder is asked about first: replace it (the document is
   updated and indexed again) or keep the old one. The zone takes exactly
   what the Knowledge Base can read — listed under the form — and refuses
   anything else, naming the file and saying why. Documents are embedded with
   the built-in semantic model (`multilingual-e5-small`, the one Grace uses):
   the first time Documents opens it fetches the model (~470 MB, once per
   installation, with a progress line), and a topic indexed before that is
   rebuilt once, on its own. With no model it still indexes, by words, and
   says so. **Preview** — or a double-click on a document — shows it in a
   window: Markdown, text, HTML and PDF as they are, Word, PowerPoint, Excel
   and OpenDocument files as their text. **Delete** removes a
   document, or a folder once nothing is left in it. To **move** a document,
   drag it onto a folder in the tree (onto empty space for the top level),
   or select it and press **Move**, then select the folder it goes to (or a
   document already in that folder) and press **Move here** — or press **To
   the top level**. The copy is indexed before the original is removed, and a
   document of the same name already there is never overwritten.
4. **Data files** — register indexed files for the topic by path (local, a
   network path, or `smb://`), each with its `.cidx`. They are only ever read.
5. **Prompt** — keep versions of the topic's system prompt; save a new one or
   bring an older one back (it asks first). **Main prompt** switches the
   screen to the prompt every topic shares: **every instruction the models
   are given**, in English, section by section — the orchestrator's
   (`=== SYSTEM ===`), the assistants' (`ASSISTANT`), the note for a topic
   with no documents (`NO SOURCES`), how a question is split into tasks and
   answered (`PLAN`, `TASK`, `COMPOSE`) and what goes with a chosen report
   template (`TEMPLATE`). Nothing is added behind it: the program only fills
   in the `{…}` words the text leaves for it. It keeps versions like a
   topic's prompt, the next question uses the active one, and **Restore
   default** saves the shipped `samples/main-prompt.md` as a new version.
6. **Chat** — ask. Past conversations are listed in the menu; pick one to
   continue it. Select any part of the conversation and copy it with
   Cmd/Ctrl+C (or the right-click menu). **Save as PDF** writes the
   conversation as a PDF with its formatting — headings, bold, lists, tables,
   code — to where you choose in the system's Save panel.

   **Reports.** Ask for a report, a presentation or an infographic and the
   chat first asks which **report template** to use. It lists them, marks
   the two or three that suit your content, and always offers **Executive**,
   the sober, professional one. The others follow the common infographic
   types: Informational, List, Timeline, Comparison, Map, Statistics,
   Flowchart, Hierarchy, Anatomical and Animated. Answer with a number or a
   name, and add any change you like ("the timeline, in green, without
   icons"), or describe a template of your own. What you change or describe
   is **saved** under its name and offered from then on. Each template is a
   complete HTML page — Bulma, a `<style>` block, no JavaScript — that the
   model fills with your content, so the report looks the same in the chat
   and in a browser. Once you choose, the model answers `TEMPLATE: <name>`
   and the chat sends it that template's page with the main prompt's
   `TEMPLATE` section. The shipped templates are in
   `samples/report-templates.txt`, and the ones in use are in
   `data/templates.idx`. Delete that file to start again from the shipped
   ones.

The flags at the foot of the menu switch the interface between English,
Portuguese, Spanish, French, Japanese and Chinese, at once.

## What is where

| Form | Does |
|---|---|
| `chat-form` (main) | The menu (designed in `forms/SideMenu-1.menu.yaml`, relabelled in the current language, shut until an agent has a model), the welcome screen, the conversation, three agents (`AGENT-1`…`AGENT-3`) with their election and orchestration, and the topic's Knowledge Base (`KB-1`); this month's token totals |
| `topics-form` | Create and open topics; install and remove the sample topics (`samples/`) |
| `documents-form` | The topic's documents as a folder tree: folders, add, move (button or drag), preview, delete, refresh |
| `settings-form` | The Knowledge Base folder, the IDE's providers, the model list and its keys, the connection test, XML export and import |
| `files-form` | The topic's registered indexed files |
| `prompts-form` | Versions of the topic's system prompt |
| `preview-form` | A document shown in a modal window, opened from `documents-form` |

PowerChat's own data is nine indexed files in `data/` (see `data/README.md`),
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
