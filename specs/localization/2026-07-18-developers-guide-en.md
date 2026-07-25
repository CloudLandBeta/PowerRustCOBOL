<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Localization work order: project-scoped AI and Grace chat

## Source and targets

- Source: `docs/developers-guide-en.md`
- Changed section: `#the-ai-assistant-optional`
- Update existing translations: `es`, `pt`, `jp`, `cn`
- Create/update the French (`fr`) guide from the canonical English source; the
  French guide does not yet exist.

## English delta

Replace language stating that AI settings and model profiles are global with
the following behavior:

> Model profiles, AI behavior, and agents belong to the open project and travel
> in its `cobolt.toml` and `agentic_ai/` directory. API keys remain
> machine-local and never travel in a repository.

Update the API-key field description to explain that keys are stored on the
machine by the project's stable model-profile id. Selecting a saved profile
restores its key; an empty field means that profile has no credential on the
current machine.

Update the Endpoint URL field to explain that untouched provider defaults
receive their conventional request path automatically. Once the developer
edits the field, the IDE uses that exact URL without appending a path.

Update the Models Manager paragraphs to state:

> Profiles are project-scoped. A profile can be reused by every agent in the
> current project. Profile metadata is stored in `cobolt.toml`; the API key is
> machine-local and keyed by profile id. Switching projects loads that
> project's profiles. Existing projects receive a one-time, non-destructive
> import of legacy global profile metadata. Missing keyed profiles can also be
> recovered from the machine's valid backup unless explicitly deleted.

Also state that **Save** commits the edited model profile and closes Models
Manager.

Change the heading label from **Agent Manager** to **Agents Manager**.

### Project-wide Grace chatbot

In `#5-the-ide-at-a-glance`, update the Main Pane description to include the
project-wide Grace chatbot opened by the **👑 Grace** button above the project
tree.

In `#the-ai-assistant-optional`, replace the former optional Grace-toggle
description with the following behavior:

- The **👑 Grace** button fills the current project-tree pane width, has a 150 px
  minimum, and follows the pane when resized.
- The button opens a project-scoped conversation in the Main Pane with persistent
  history, workflow progress, and approval controls for gated operations.
- Every IDE chatbot routes through Grace with an advisory surface preference.
  The Form Designer prefers the Form Designer Agent, the event editor prefers
  the COBOL Event Handler Script Agent, and the code editor lets Grace select by
  capability.
- A preference never restricts delegation. Grace can coordinate any enabled
  specialists for mixed work such as creating a control and wiring `onClick`.
- Workflow records remain under `agentic_ai/Grace/runs/`.

Add the `project-grace-chat.png` screenshot placeholder. Screenshots are shared
and require no translated asset.

### Connection-test temperature

Update the **Temperature** field description to state that connection tests use
the configured value exactly. Some models accept only the provider-defined
default, commonly `1.0`.

### Form Designer window activation

In `#7-the-form-designer-rad`, state that double-clicking a form in either the
IDE project tree or a designer's **Forms** list opens it. If its designer window
is already open, the IDE restores it and brings it to the front.

### Grace welcome and indexed project documentation

In `#the-ai-assistant-optional`, document these additions:

- An empty project-wide Grace conversation shows examples for Indexed Files,
  CRUD forms, data-bound DataGrids, ERP planning, task creation, and task
  implementation.
- `Documentation Agent` is a fixed, non-deletable project specialist and the
  only agent allowed to format, create, or update project documentation.
- Domain specialists prepare authoritative source material. Grace makes the
  Documentation Agent task depend on those source tasks, and approved outputs
  are passed to the Documentation Agent. For interface documentation, the Form
  Designer Agent first reports controls, layout, bindings, and events; the
  Documentation Agent then formats and saves the document without inventing
  missing facts.
- Grace validates the documentation task structure before execution and asks
  for one corrected plan if document ownership or required source dependencies
  are wrong.
- The project tree hides the internal `agentic_ai/` directory. Agent
  configuration remains available through Agents Manager, and Grace continues
  to keep workflow records in that on-disk directory.
- The Agents Manager prompt editor is vertically resizable from four to twenty
  text rows. Longer prompts scroll inside the editor and never increase it
  beyond the twenty-row maximum.
- The project-wide Grace property-pane header reads
  `👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator`. Keep this product
  title unchanged in translations.
- Every new project receives a protected `Grace Pedantic Reviewer Agent`, linked as
  Grace's companion and seeded with its orchestration-review prompt. The prompt
  is project-local and editable in Agents Manager; fixed-agent repair preserves
  non-empty developer edits. The reviewer needs a model different from Grace's
  model before its connection is active.
- A Pedantic agent now exposes an editable `Pedantic Companion for` selector
  listing eligible orchestrators and specialists. Companion relationships are
  one-to-one and can be edited from either side; Grace and participating agents
  receive the exact project relationship in their runtime instructions.
- The canonical names are `Documentation Agent` and
  `Grace Pedantic Reviewer Agent`. Existing aliases are migrated without
  replacing stable IDs, model profiles, companion links, or custom prompts;
  `Orchestrator Pedantic Reviewer Agent` is merged and removed.
- Form Designer Agent, COBOL Event Handler Script Agent, Documentation Agent,
  Grace Pedantic Reviewer Agent, and Version Control Agent now have explicit
  role-specific routing and default prompts. Empty and known legacy defaults
  are repaired while non-empty project-edited prompts remain authoritative.
- Documentation Agent may create, read, and list text documents only under
  `Documentation/` or the existing `docs/` tree.
- Documents created by Grace are immediately tracked in the project tree and
  indexed in the project-local SQLite vector database at
  `data/project-knowledge.sqlite`.
- The IDE synchronizes user-added textual documentation before every Grace
  workflow. Relevant excerpts are included in planning, and specialists use
  governed, read-only `knowledge.search` retrieval for prior plans,
  requirements, task lists, and project decisions.
- Grace answers capability/help questions directly without creating a workflow.
  Actionable requests still require workflow JSON and receive one correction
  attempt when malformed. Named project agents receive only their project
  prompts, never an unrelated built-in specialist preamble; repeated plan
  failures report both parser errors and the complete corrected payload.
- Every chatbot composer keeps **Send** immediately to the right of its prompt.
  The prompt consumes the remaining width as the pane resizes, and multiline
  composers never move Send to a row below.

## Translation rules

- Do not translate `PowerRustCOBOL`, `Models Manager`, `Agents Manager`,
  `Grace`, `Grace Pedantic Reviewer Agent`, `Documentation Agent`, `Form Designer Agent`, `COBOL Event Handler Script Agent`,
  `cobolt.toml`, `agentic_ai/`, `onClick`, API identifiers, COBOL keywords,
  identifiers, or code samples.
- Preserve Markdown links, anchors, tables, inline code, and emphasis.
- Translate only the changed section; do not rewrite unaffected content.
- Screenshots are shared and language-neutral. No screenshot work is required.
