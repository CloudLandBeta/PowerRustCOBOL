<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Configure Claude Code

- **Status:** approved (operator, 2026-10-02)
- **Folder:** specs/084-configure-claude-code/
- **Author:** Claude (for the operator)   **Date:** 2026-10-02
- **Supersedes:** the per-project export of spec 080 (R1–R6, R15, the reference
  pack of R7, and the project-folder file layout of R2). Spec 080's tool set
  (`cobolt-project-tools`), its rules (R8, R16–R19), its skills (R9) and its
  reviewer (R10) are kept and re-delivered as described here.

## 1. Overview

Spec 080 made **Export coding-agent kit → Claude Code** write a kit into each
project folder: `CLAUDE.md`, `.claude/settings.json`, `.claude/skills/`,
`.claude/agents/`, `.mcp.json` and a copy of the reference documentation under
`docs/powerrustcobol/`. A fix to any of it therefore had to be re-exported into
every project, and every project carried its own stale copy of the
documentation.

This feature replaces that with a one-time, IDE-wide integration. **Configure
Claude Code** sets up the developer's existing Claude Code installation once, at
user level, by installing a Claude Code **plugin** shipped with the IDE. The
plugin carries the skills, the reviewer and the connection to the IDE. The
rules and the knowledge are served live by the IDE itself, so Claude Code always
reads the IDE's current Knowledge Base instead of a copied file. Nothing is
written into project folders. Claude Code can create a project, open one, and
edit the project open in the IDE. IDE-wide settings for the integration live in
a new **Claude Code Settings** window in the Help menu.

It is delivered in three phases. **Phase A** is the integration itself
(R1–R29, R35–R38). **Phase B** gives Claude Code eyes and hands on a running
form: `render_form` and `run_form` (R30–R31). **Phase C** gives it the
application-level knowledge an application like PowerDemo3 or PowerChat needs:
three skills, a patterns pack and an architecture map (R32–R34).

## 2. Goals / Non-goals

- **Goals:**
  - One configuration per machine: an IDE update reaches every project without
    re-exporting anything.
  - Claude Code reads the IDE's live Knowledge Base, never a copied reference.
  - Claude Code can create a new project, open an existing one, and edit the
    project currently open in the IDE.
  - Configuration uses Claude Code's own CLI, never hand-edits its files.
  - Nothing in the configuration changes how Claude Code behaves in projects
    that are not PowerRustCOBOL projects.
  - Claude Code can see what it built (a picture of a form) and exercise it (run
    a form with scripted events), not only prove that it compiles.
  - Claude Code can build a multi-form application in the manner of PowerDemo3
    or PowerChat from the knowledge it is given.
- **Non-goals:**
  - Coding agents other than Claude Code. The content stays agent-neutral
    (spec 080 R20) so a later spec can add one.
  - Migrating per-project kits. No project has one yet (operator, 2026-10-02).
  - The "Export AI…" button in Project Settings (agents and models export). It
    is a different feature and is unchanged.
  - Driving the IDE's own user interface. `run_form` runs a form, not the IDE.

## 3. User stories

- As a developer, I want to configure Claude Code once from the IDE, so that
  every PowerRustCOBOL project works with it without a per-project export.
- As a developer, I want an IDE update to update what Claude Code knows, so that
  a fix to the rules or skills reaches every project at once.
- As a developer, I want to ask Claude Code to create a new project or open an
  existing one, so that I can start work from Claude Code as well as from the IDE.
- As a developer, I want Claude Code to look things up in the IDE's Knowledge
  Base, so that it never works from a stale copy of the documentation.
- As a developer, I want one place in the IDE to see whether Claude Code is
  configured, on which port, and whether the plugin is current.
- As a developer who uses Claude Code for other work, I want this integration
  not to change Claude Code's behaviour outside PowerRustCOBOL projects.
- As a developer, I want Claude Code to look at and try the forms it builds, so
  that a layout or behaviour mistake is caught before I open the form myself.
- As a developer, I want to ask Claude Code for a whole application — a main
  form with a side menu, forms loaded into its content pane, data files, web
  calls — and get one that follows the product's own patterns.

## 4. Requirements (EARS)

### Configuring Claude Code

- **R1 (event):** When the developer chooses **File → Configure Claude Code**,
  the IDE shall install or update the PowerRustCOBOL plugin in the developer's
  Claude Code at user scope, and report the outcome. This replaces the
  **Export coding-agent kit** menu item, which shall be removed.
- **R2 (constraint):** The IDE shall configure Claude Code only through Claude
  Code's own command-line interface. It shall not write Claude Code's
  configuration files (`~/.claude.json`, `~/.claude/settings.json`) directly.
- **R3 (event):** When Claude Code is not installed or its command cannot be
  run, Configure Claude Code shall say so, name what it looked for, and change
  nothing.
- **R4 (event):** When a Claude Code command fails, the IDE shall show the
  command's own message and leave any earlier configuration as it was.
- **R5 (constraint):** Configure Claude Code shall write nothing into any project
  folder.
- **R6 (ubiquitous):** The plugin shall be shipped with the IDE and carry the
  IDE's version. Configuring again with the same version shall change nothing.

### The plugin

- **R7 (ubiquitous):** The plugin shall contain the spec 080 skills (R9) and the
  reviewer (R10), and the connection to both tool servers: the IDE over HTTP,
  and `rcrun mcp` over stdio for when the IDE is closed.
- **R8 (constraint):** The plugin shall pre-approve the PowerRustCOBOL project
  tools on both servers, where Claude Code lets a plugin do so (confirmed in plan
  step A0; if it does not, the developer approves each tool once and this is
  recorded as a deviation). It shall not deny or restrict any other Claude Code
  tool, Bash included, because user-scope rules apply to every project.
- **R9 (constraint):** The plugin shall not install a user-level `CLAUDE.md` or
  any other instruction that applies outside PowerRustCOBOL projects.
- **R10 (constraint):** The plugin shall carry no secret and nothing personal:
  no API key, no user name, no home-folder path (spec 080 R4). The access token
  of R35 is not part of the plugin bundle; it is given to Claude Code's
  server definition at configuration time.

### Rules and knowledge, served live

- **R11 (ubiquitous):** Both tool servers shall give Claude Code the standing
  rules (spec 080 R8, R16–R19, and the tool rules) as their MCP server
  instructions, generated from the running IDE or `rcrun` version.
- **R12 (ubiquitous):** Both tool servers shall answer Knowledge Base questions
  from the same System KB the IDE uses: `kb_lookup` (one name) and a new
  `kb_search` (free text, returning the matching records).
- **R13 (ubiquitous):** Both tool servers shall expose the reference documents
  spec 080 R7 copied into the project — the Developer's Guide, the supported
  COBOL-85 syntax, the KB documents, the built-ins list and the `.cfrm` / `.cidx`
  format descriptions — as MCP resources, read from the running binary.
- **R14 (constraint):** No reference document shall be copied into a project
  folder.

### Projects

- **R15 (event):** When Claude Code calls `create_project` with a folder and a
  name, the tool shall create a new PowerRustCOBOL project there, exactly as
  the IDE's New Project does, and open it in the IDE when the IDE is running.
- **R16 (event):** When Claude Code calls `open_project` with a project folder or
  manifest, the IDE shall open that project as File → Open Project does.
- **R17 (state):** While the IDE has unsaved changes in its current project,
  `create_project` and `open_project` shall refuse, name the unsaved files, and
  open nothing.
- **R18 (constraint):** `create_project` shall refuse a folder that already
  holds files, and shall never overwrite anything.
- **R19 (ubiquitous):** Every project tool answer shall name the project it
  acted on.
- **R20 (event):** When a project tool is called with a project that is not the
  one open in the IDE, the IDE server shall refuse and name both projects, so
  that Claude Code opens the right one or asks the developer. It shall never act
  on a project other than the one open.
- **R20a (event):** When `open_project` or `create_project` is called through
  `rcrun mcp` while the IDE is not running, `rcrun` shall launch PowerRustCOBOL
  AI with that project open, wait until the IDE's tool server answers, and
  report that later calls should go to the IDE server. Where the IDE cannot be
  found or does not answer in time, it shall say so; `create_project` shall
  still have created the project.
- **R21 (event):** When `rcrun mcp` starts without `--project`, it shall find the
  project from its working directory, searching upward for a project manifest
  (`*.project.toml`, or `cobolt.toml` in older projects). Where none is found,
  `create_project` and `open_project` shall still work and the other tools shall
  say no project is open.

### Claude Code Settings

- **R22 (ubiquitous):** The Help menu shall carry **Claude Code Settings**, an
  IDE-wide window in the manner of Debug Settings.
- **R23 (ubiquitous):** Claude Code Settings shall show whether Claude Code was
  found, whether the plugin is installed and at which version, and the tool
  server's address, with a button to configure or update.
- **R24 (ubiquitous):** The port the IDE serves the tools on shall be set in
  Claude Code Settings. It is already a machine-wide value (`LlmConfig.mcp_port`);
  its **Coding-agent tools port** row moves out of the project Settings form.
- **R25 (event):** When the port is changed, the IDE shall serve on the new port
  and tell the developer to configure Claude Code again.
- **R26 (state):** While the installed plugin's version differs from the IDE's,
  Claude Code Settings shall say so and offer to update it.
- **R27 (constraint):** The Claude Code Settings window shall never resize
  itself (CONVENTIONS.md: an explicit stored size, one grip).

### Access

- **R35 (ubiquitous):** Configure Claude Code shall generate an access token for
  this machine, keep it in the IDE-wide settings, and give it to Claude Code's
  definition of the IDE server, which sends it with every call.
- **R36 (event):** When a call reaches the IDE's tool server without the current
  token, the IDE shall refuse it and do nothing else.
- **R37 (event):** When the developer chooses **Renew token** in Claude Code
  Settings, the IDE shall replace the token and tell the developer to configure
  Claude Code again.
- **R38 (constraint):** The token shall never appear in a log, the Output pane,
  a project file or a tool answer.

### Phase B — seeing and running a form

- **R30 (event):** When Claude Code calls `render_form` with a form of the open
  project, the tool shall return a PNG of the form as it renders at run time —
  the same engine, theme and designed size as Run Form — optionally at a given
  scale, and, for a form shown in a shell's ContentPane, optionally inside that
  shell.
- **R31 (event):** When Claude Code calls `run_form` with a form and a list of
  steps — set a control's property, raise an event on a control, wait — the tool
  shall run the form in the real form host, off screen, apply the steps in
  order, and return what the form displayed, the properties the steps asked to
  read, every runtime error, and a PNG of the final state. A run shall stop
  after a bounded time and say so.
  - The form runs the developer's own code with its real effects (files under
    the project, web calls). The tool's description shall say so.
  - `run_form` shall not open a visible window and shall not drive the IDE.

### Phase C — building an application

- **R32 (ubiquitous):** The plugin shall add three skills: **build an
  application** (the structure of a multi-form application and the order of
  work), **shell and navigation** (main form with SideMenu or MenuBar, forms
  loaded into the ContentPane, menus, child forms, `super::`, breadcrumbs), and
  **layout and themes** (responsive anchoring, form themes, the visual checks
  to make with `render_form`).
- **R33 (ubiquitous):** Both tool servers shall serve a **patterns pack** as MCP
  resources: forms taken from the PowerDemo3 and PowerChat examples, each with
  a note on what it demonstrates — at least the application shell, a form in
  the ContentPane, an indexed-file maintenance form, a web-service call and a
  chat with an AgentObject. The pack is read from the binary, and a test shall
  prove every pattern loads and passes `check`.
- **R34 (ubiquitous):** The server instructions (R11) shall include an
  application-architecture section and a map of which resource or tool answers
  which kind of question.

### Unchanged from spec 080

- **R28 (constraint):** The tools shall stay on `127.0.0.1` (spec 080 R12) and
  expose nothing outside the open project's folder except `create_project` and
  `open_project` (spec 080 R13, extended by R15–R16).
- **R29 (ubiquitous):** Every new IDE string shall be translated in all six
  languages. The text Claude Code reads (instructions, skills, resources) stays
  English (spec 080 R22).

## 5. Acceptance criteria

- [ ] **AC1** (R1, R5, R6) — On a machine with Claude Code, File → Configure
  Claude Code installs the plugin. `claude plugin` lists it at the IDE's
  version, and no file was created or changed in the open project folder.
  Running it a second time reports that nothing changed.
- [ ] **AC2** (R2) — The configuration path runs only `claude` commands. A test
  with a stub `claude` records the exact commands, and no write to
  `~/.claude.json` or `~/.claude/settings.json` happens.
- [ ] **AC3** (R3, R4) — With `claude` missing from the path, the action reports
  it and changes nothing. With a stub `claude` that fails, the IDE shows the
  stub's message.
- [ ] **AC4** (R7, R8, R9, R10) — The plugin bundle holds the seven skills, the
  reviewer and both server definitions. Its permissions allow the project tools
  and deny nothing. It holds no `CLAUDE.md`, and the spec 080 redaction checks
  pass over every file in it.
- [ ] **AC5** (R11) — `initialize` on both servers returns instructions that
  contain every spec 080 R8 rule id's text and the running version.
- [ ] **AC6** (R12) — `kb_search` with "SideMenu ContentPane" returns records
  that name both, from the same store the IDE's assistant uses.
- [ ] **AC7** (R13, R14) — `resources/list` on both servers lists the reference
  documents, and `resources/read` returns the same text as the binary's
  embedded copy.
- [ ] **AC8** (R15, R18) — `create_project` in an empty temporary folder makes a
  project that `check` accepts. In a folder holding a file, it refuses and
  nothing changes.
- [ ] **AC9** (R16, R17) — `open_project` opens a project in the IDE. With an
  unsaved form open, it refuses and names the form.
- [ ] **AC10** (R19, R20) — Every tool answer names its project. A call naming
  another project than the open one is refused with both names.
- [ ] **AC11** (R21) — `rcrun mcp` started in a project's sub-folder, with no
  `--project`, serves that project. Started in a folder with no project above
  it, `create_project` works and `check` says no project is open.
- [ ] **AC12** (R22–R27) — Help → Claude Code Settings shows the detected
  Claude Code, the plugin version and the address. Changing the port moves the
  server and prompts to configure again. Dragging nothing leaves the window's
  size unchanged across frames.
- [ ] **AC13** (R24) — The project Settings form no longer shows Coding-agent
  tools port; Claude Code Settings shows and changes the same machine-wide value.
- [ ] **AC14** (R29) — Every new `Tr` field exists in all six languages
  (`i18n_tests`).
- [ ] **AC15** (R20a) — With the IDE closed, `open_project` through `rcrun mcp`
  starts the IDE with that project open and its tool server answers. With the
  IDE binary not found, the answer says so and `create_project` has still
  created the project.
- [ ] **AC16** (R35–R38) — A call to the IDE server without the token, or with
  an old one after Renew token, is refused. A search of the logs, the Output
  pane text and the project folder after a session finds no token.
- [ ] **AC17** (R30) — `render_form` on a PowerDemo3 form returns a PNG whose
  pixels match the Run Form golden render of the same form (spec 056 goldens),
  and a ContentPane form rendered inside its shell shows the side menu.
- [ ] **AC18** (R31) — `run_form` on a test form with a button whose handler
  sets a label returns the label's new Caption, the handler's DISPLAY line and a
  PNG. A handler that loops forever is stopped at the time limit and reported.
- [ ] **AC19** (R32, R34) — The plugin holds the three new skills; the
  instructions carry the architecture section and the resource map.
- [ ] **AC20** (R33) — `resources/list` lists the patterns pack with at least the
  five named patterns, and the pattern test passes.

## 6. Constraints & steering check

- **i18n (6 languages):** yes. The menu item, the Claude Code Settings window
  and every outcome message (R29).
- **Generated-code / regenerate contract:** unchanged. `regenerate` and
  `check` keep their spec 080 behaviour; `create_project` writes what New
  Project writes.
- **Docs (English guide):** yes. The guide's coding-agent section (spec 080
  Phase 6) is rewritten for Configure Claude Code and Claude Code Settings;
  under GOLDEN RULE #8 its five translations are deleted in the same change.
- **System KB:** no control, property, method or event changes, so the
  compiler doc tables are untouched. `kb_search` reads the existing store.
- **Window rule:** Claude Code Settings follows the leaderboard pattern (R27).
- **Fix vs feature:** **feature** — a new IDE integration and new tools. Every
  change bumps `z` only.
- **Form host parity:** `run_form` uses the one form host (spec 042), so what it
  reports is what `rcrun run-form` and a compiled application do.
- **Never drive the application:** `run_form` runs a form off screen through
  the form host; it is not UI automation of the IDE, which CLAUDE.md forbids
  for verification.
- **Facts this spec relies on, and how they were checked:**
  - A stdio MCP server is started in the folder Claude Code was launched from,
    and receives `CLAUDE_PROJECT_DIR` set to that folder. Tested 2026-10-02
    with Claude Code 2.1.158 and a probe server loaded with `--mcp-config`.
  - User-scope servers, plugins (skills, agents, MCP servers), MCP server
    instructions and MCP resources are Claude Code features. Their exact CLI
    commands and file layout are to be confirmed against the installed Claude
    Code in `/plan` before any design depends on them. A documentation lookup
    on 2026-10-02 was wrong about `CLAUDE_PROJECT_DIR`, so none of its answers
    is relied on unverified.

## 7. Decisions (were open questions, operator 2026-10-02)

- **Q1 — Unsaved changes on create/open:** refuse and name the unsaved files
  (R17).
- **Q2 — IDE not running:** `open_project` and `create_project` through `rcrun`
  launch the IDE with the project open (R20a).
- **Q3 — Who may call the HTTP server:** `127.0.0.1` plus an access token the
  IDE generates at configuration time (R35–R38).
- **Q4 — The follow-up capabilities:** folded into this spec as Phase B
  (`render_form`, `run_form`) and Phase C (skills, patterns pack, architecture
  map).

## 8. Open questions

- None outstanding. `/plan` must first confirm the Claude Code facts listed in
  section 6 against the installed version.
