<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Configure Claude Code

- **Status:** Phases A–C approved (operator, 2026-10-02) and shipped. **Phase D
  draft** (operator's rule of 2026-11-18, below), awaiting approval.
- **Folder:** specs/084-configure-claude-code/
- **Author:** Claude (for the operator)   **Date:** 2026-10-02; Phase D
  2026-11-18
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

It is delivered in four phases. **Phase A** is the integration itself
(R1–R29, R35–R38). **Phase B** gives Claude Code eyes and hands on a running
form: `render_form` and `run_form` (R30–R31). **Phase C** gives it the
application-level knowledge an application like PowerDemo3 or PowerChat needs:
three skills, a patterns pack and an architecture map (R32–R34).

**Phase D** (R44–R73) makes the MCP connection the only channel. The operator's
rule, 2026-11-18: *"I do not want anything outside the MCP, except Skills added
to Claude Code by the PowerRustCOBOL AI 'Configure Claude Code' option.
Everything else goes through the MCP connection."* An audit of 2.0.611 found
the connection itself was MCP only, but the skills sent Claude Code to its own
file tools to write forms, COBOL, indexed definitions, menus and assets, and
most of what the IDE can do had no tool. Phase D gives every such action a
tool — the files, the Form Designer, the project and IDE-wide settings, run and
debug, the knowledge bases, the IDE's own windows — rewrites the skills and
instructions to use only the tools, reduces the plugin to skills plus the
connection, and has the IDE report a project file that changed outside the
connection.

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
  - **Phase D:** Claude Code reaches PowerRustCOBOL AI only through the MCP
    connection. The plugin adds nothing to Claude Code but skills and that
    connection.
  - **Phase D:** everything the developer can do to a project in the IDE, Claude
    Code can do through a tool. That covers files, forms, settings, run, debug,
    knowledge bases and git, the IDE-wide settings, and the IDE's own windows.
- **Non-goals:**
  - Coding agents other than Claude Code. The content stays agent-neutral
    (spec 080 R20) so a later spec can add one.
  - Migrating per-project kits. No project has one yet (operator, 2026-10-02).
  - The "Export AI…" button in Project Settings (agents and models export). It
    is a different feature and is unchanged.
  - *(Removed 2026-11-18: "Driving the IDE's own user interface." The operator
    put it in scope; it is R70–R72 of Phase D, through the MCP connection.
    `run_form` itself still runs a form, not the IDE.)*
  - **Grace and the IDE's AI agents** (asking Grace or a specialist, the agents
    database, model settings). They were not chosen for Phase D (operator,
    2026-11-18); a later amendment can add them.
  - **The egui inspection endpoint** on `127.0.0.1:5719` stays as it is today
    (operator, 2026-11-18). Phase D neither changes it nor relies on it.
  - **Restricting Claude Code's own tools.** R8 stands, and R44 allows no hook,
    so Claude Code is not stopped from using its file or shell tools. Phase D
    makes them unnecessary (R45–R47) and makes their use visible (R68).

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
- As a developer, I want every change Claude Code makes to my project to go
  through PowerRustCOBOL AI's own tools. Then the IDE stays the one writer of
  the project, sees each change, and shows it as Claude makes it.
- As a developer, I want Claude Code to be able to do anything I can do in the
  IDE: design a form, change a setting, run, debug, look at the IDE. Then I
  never have to finish a task by hand because a tool is missing.
- As a developer, I want to be told when a project file changed outside the
  MCP connection, so that a change Claude Code made with its own tools does not
  go unnoticed.

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
  and `rcrun mcp` over stdio for when the IDE is closed. *(Amended by R44: the
  reviewer is delivered as a skill.)*
- **R8 (constraint):** The plugin shall pre-approve the PowerRustCOBOL project
  tools on both servers, where Claude Code lets a plugin do so. **Deviation
  (T1 spike, 2026-10-02):** Claude Code 2.1.158 offers no plugin-level or CLI
  permission rule, so the developer approves each tool once in Claude Code. It shall not deny or restrict any other Claude Code
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

### First-run wizard (operator, 2026-10-03; mockup approved)

- **R39 (event):** When the IDE starts and no coding agent is connected, and the
  developer has not chosen "Don't show again" for it, the IDE shall show the
  **Connect a coding agent** window, once per start, after the background check
  answers.
- **R40 (ubiquitous):** The window shall be 800 × 400 and never resize: the
  PowerRustCOBOL mascot at a laptop facing a coding agent at its own, a
  connection being made between them, on the left; on the right the title with
  an agent picker beside it (Claude Code only for now), the lead text, three
  points and the detection status; **Don't show again**, **Skip for now** and
  **Connect** 10 px from the bottom and right borders.
- **R41 (event):** **Connect** runs Configure for the picked agent (disabled
  while it is not installed); **Skip for now** closes until the next start;
  **Don't show again** records the agent in the IDE-wide settings.
- **R42 (ubiquitous):** Agents are a list the wizard reads (name, key,
  detection, configuration), so another agent is an entry, not a new window.
- **R43 (ubiquitous):** Help ▸ Claude Code Settings is named **Coding Agent
  Settings**.

### Phase D — the MCP connection is the only channel (operator, 2026-11-18)

Unless a requirement says otherwise, each tool below is served by both tool
servers. A tool that needs the running IDE (the designer's live state, the
debugger, the IDE's windows and settings) is served through the IDE as R66
says.

**The channel**

- **R44 (constraint):** The plugin shall carry only skills and the definition
  of the two tool servers, with the port and token settings that definition
  needs. It shall carry no agent, hook, command, output style, settings file or
  any other Claude Code component. The reviewer of R7 shall be a skill.
- **R45 (ubiquitous):** Every action Claude Code needs on a PowerRustCOBOL
  project or on PowerRustCOBOL AI shall have an MCP tool or resource. That
  includes reading, searching, creating, changing, renaming, moving or
  removing a file; checking, building, running and debugging; looking anything
  up; and seeing or acting on the IDE.
- **R46 (constraint):** No skill, server instruction, tool description, tool
  answer or resource shall tell Claude Code to use its own file, search or
  shell tools, or any program, to act on a project or on PowerRustCOBOL AI.
  Every step of a skill that acts on the project shall name the tool that
  carries it out.
- **R47 (ubiquitous):** The server instructions shall state the rule: the
  project and the product are reached only through these tools and resources.
  When a capability has no tool, Claude Code shall report the gap with the
  gap-report tool (R65) and stop, not work around it.
- **R48 (constraint):** The rule is enforced by R45–R47 and made visible by
  R68, not by restricting Claude Code. R8 stands.

**Project files**

- **R49 (event):** Claude Code shall be able to read any file of the project.
  A text file comes whole or as a range of lines. A binary file comes with its
  type and its bytes in a form MCP can carry. Claude Code shall also be able
  to list a folder, and to search the project's files by text or by pattern.
  A search answers with the file, the line and the matching text.
- **R50 (event):** Claude Code shall be able to create a file, replace a
  file's content, and change a file by replacing an exact piece of its text.
  The answer says what changed. A file of a kind the project lists (a form,
  Common Code, an indexed definition, an asset) is recorded in the project as
  `add_to_project` records it.
- **R51 (event):** Claude Code shall be able to rename or move a file or a
  folder. Every place the IDE's own rename updates (the manifest, the
  main-form seal, references) is updated the same way.
- **R52 (event):** Claude Code shall be able to remove a file or a folder. The
  developer shall be able to recover it, and the answer says where it went.
  (User code is never lost.)
- **R53 (event):** Claude Code shall be able to import a file the developer
  names, from outside the project, into `Assets/` or another project folder.
  Binary files arrive unchanged.
- **R54 (constraint):** The file tools shall not write generated code
  (`generated/`, generated copybooks), which only `regenerate` and the Build
  write. They shall not write the manifest, which stays sealed and is written
  through the tools that change the project. They shall refuse any path
  outside the project folder, except the source of an import (R53), which is
  only read.
- **R55 (state):** While the IDE holds unsaved edits in a file, every tool that
  would write that file shall refuse and name it. This includes
  `set_definition`, which today is not checked.

**Forms, menus and indexed definitions**

- **R56 (event):** Claude Code shall be able to do to a form everything the
  Form Designer offers, sent as one change set that applies whole or not at
  all. That includes creating a form; adding, copying, deleting, moving,
  resizing and renaming controls; properties; data bindings; tab order;
  layers; events and their handlers; procedures; the form's COBOL structure
  blocks; animations; DataGrid columns; toolbars; menu items; and a web form's
  stylesheet. The validation and the control identities shall be the ones the
  designer uses.
- **R57 (state):** While the form is open in a designer, a change set shall
  appear in it at once and join its undo history.
- **R58 (event):** Claude Code shall be able to create and change an indexed
  definition (`.cidx`) field by field, as the IDE's definition editor does.

**The project**

- **R59 (event):** Claude Code shall be able to read and change every field
  of Project Settings, through the IDE's project model, which saves and seals
  the project. That includes its connections and the Backend's exports. A
  secret (a password, a key) goes where the IDE keeps it, and no tool answer
  ever contains one (as R38).
- **R60 (event):** Claude Code shall be able to create, change, remove and
  test a SQL connection. The test answers with success or the database's own
  error, and never with a password.
- **R61 (event):** Claude Code shall be able to save the project, package it,
  and upgrade a project made by an older version, as the IDE does.
- **R62 (event):** Claude Code shall be able to run the project as Run does,
  read its output and exit status, and stop it. It shall be able to start and
  stop a Web or Mobile project's Backend, with its address and whether it
  answers. It shall be able to start the Web Preview and get its address, run
  a Mobile project on an Android device or emulator or on the iOS simulator,
  and create the Android Studio project.
- **R63 (event):** Claude Code shall be able to debug: start a session, set
  and clear breakpoints, continue, pause, step over, into and out, read the
  call stack, variables and watches, and stop. It is the debugger the IDE
  shows, and while the IDE is open the session is visible in it.
- **R64 (event):** Claude Code shall be able to:
  - search and reindex the Project KB, and search the Application KB;
  - browse, find, add, change and remove the records of an indexed file, as
    the record grid does;
  - read the Output panel, the diagnostics log and the Run Form inspector;
  - beautify a COBOL source or a form's code, as Beautify does;
  - use git on the project: status, diff, commit, push, pull and branches.
  The developer's own git credentials are used and never appear in an answer.
- **R65 (event):** Claude Code shall be able to write a gap report through a
  tool. The report goes where the gap-report skill says, in its format.

**Both servers**

- **R66 (state):** While the IDE is running, `rcrun mcp` shall pass every call
  to the IDE's tool server and answer with its answer. The IDE then stays the
  only writer of the open project and sees every call; the mascot of 2.0.612
  shows them. While the IDE is closed, `rcrun mcp` serves every tool that does
  not need the IDE. A tool that needs it answers that PowerRustCOBOL AI must
  be open, and how to open it (R20a).
- **R67 (ubiquitous):** Each tool shall say in its description whether it
  changes anything, and what. A tool that runs the developer's code shall say
  so, as `run_form` does.

**Changes outside the connection**

- **R68 (event):** While Claude Code is connected (as the mascot defines it),
  when a file in the open project changes on disk and neither a tool nor the
  IDE wrote it, the IDE shall do two things. It shall write an Output line
  naming the file. Its next tool answer shall name the file, say that it
  changed outside the MCP connection, and say that the change must be made
  through the tools. It reports and does not refuse (operator, 2026-11-18). A
  change the developer makes in another editor is reported the same way.

**IDE-wide settings and the IDE itself** (operator, 2026-11-18)

- **R69 (event):** Claude Code shall be able to read and change the IDE-wide
  settings: the IDE theme, the default form theme, the IDE's language, Debug
  Settings, the Build Folder and the Platform SDK paths. The tool server's
  port and token are not among them: they stay the developer's.
- **R70 (event):** Claude Code shall be able to see the IDE: the open project,
  the open tabs and windows and which one is active, the controls selected in
  a designer, the panels shown, and a picture of the IDE window or of one panel.
- **R71 (event):** Claude Code shall be able to act on the IDE through its own
  operations, each by name. It shall be able to open, activate and close a
  designer, editor or document tab; go to a line; select controls; switch a
  designer layer; show or hide a panel; and carry out any entry of the File,
  Settings, Run, View and Help menus. For what no named operation covers, it
  shall be able to point at, click, type into and scroll an IDE widget, served
  only through the token-gated connection.
- **R72 (constraint):** No UI tool shall discard unsaved work. A call that would
  close a tab, a window or the project with unsaved changes refuses and names
  them, as R17 does.

**Coverage**

- **R73 (ubiquitous):** One list shall map every IDE capability to the tool or
  resource that reaches it: each menu entry, each Form Designer operation, each
  Project Settings field, each panel. A test shall fail when the list leaves
  one of them without a tool.

### Unchanged from spec 080

- **R28 (constraint):** The tools shall stay on `127.0.0.1` (spec 080 R12) and
  expose nothing outside the open project's folder except `create_project` and
  `open_project` (spec 080 R13, extended by R15–R16). *(Phase D adds the source
  of an import (R53, read only), the IDE-wide settings (R69) and the IDE itself
  (R70–R72).)*
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
- [ ] **AC17** (R30) — `render_form` on a PowerDemo3 form returns a PNG of the
  form's designed size, drawn by the same engine path the spec 056 Run goldens
  pin (theme, backdrop, live properties), byte-identical on a second call; and
  a ContentPane form rendered inside its shell shows the side menu. *(The
  goldens hash shapes, not pixels, so a pixel comparison with them is not
  possible — corrected 2026-10-03.)*
- [ ] **AC18** (R31) — `run_form` on a test form with a button whose handler
  sets a label returns the label's new Caption, the handler's DISPLAY line and a
  PNG. A handler that loops forever is stopped at the time limit and reported.
- [ ] **AC19** (R32, R34) — The plugin holds the three new skills; the
  instructions carry the architecture section and the resource map.
- [ ] **AC20** (R33) — `resources/list` lists the patterns pack with at least the
  five named patterns, and the pattern test passes.

**Phase D**

- [ ] **AC21** (R44) — The plugin bundle holds only `skills/`, the server
  definitions and the plugin manifest. It has no `agents/`, `hooks/`,
  `commands/` or settings. The reviewer is a skill.
- [ ] **AC22** (R46, R47) — A test reads every skill, the server instructions,
  every tool description and every resource. It finds no step that sends Claude
  Code to its own Read, Write, Edit, Glob, Grep or Bash tools, or to a shell
  command, to act on a project. It finds the MCP-only rule in the instructions.
- [ ] **AC23** (R49–R54) — In a temporary project, the file tools do the
  following:
  - read a form whole and a range of lines;
  - search for a paragraph name and get file and line;
  - create Common Code and find it recorded in the manifest;
  - change a handler by an exact replacement;
  - rename a form and find the manifest and seal updated;
  - remove a file and recover it from where the answer says;
  - import a PNG byte for byte.
  
  A write into `generated/`, the manifest, or `../` is refused.
- [ ] **AC24** (R55) — With a form held unsaved in the IDE, `set_definition`
  and every file tool refuse to write it and name it.
- [ ] **AC25** (R56, R57) — A change set that adds a Button, sets its Caption,
  binds `onClick` to a new handler and adds a menu item is applied in one call.
  The form then passes `check`. A form open in a designer shows the change and
  undoes it in one step. A change set with one invalid operation changes
  nothing and names the operation.
- [ ] **AC26** (R58–R61) — An indexed definition built field by field passes
  `check`. A Project Settings field changed by a tool is the value the Settings
  form shows, and the project is still sealed. A SQL connection created and
  tested by a tool reports success against the test database, and no answer
  contains its password. Save, Package and Upgrade each do what the menu does.
- [ ] **AC27** (R62) — Run on a console project returns its DISPLAY lines and
  exit status. A run that does not end is stopped by a tool. A Web project's
  Backend started by a tool answers at the address returned, and the Web
  Preview's address serves the page.
- [ ] **AC28** (R63) — A debug session stops at a breakpoint, steps over a
  line, reads a variable's value and continues to the end. While the IDE is
  open, its debugger shows the same line.
- [ ] **AC29** (R64, R65) — Project KB search finds a document added to the
  project's Knowledge Base after a reindex. Record tools add, find and remove
  a record of an indexed file. The Output text read by a tool contains a line
  the IDE just wrote. Beautify through a tool gives the same text as the IDE's
  Beautify. A git commit through a tool appears in `git log`. A gap report
  through a tool is written where the skill says.
- [ ] **AC30** (R66) — With the IDE running, a call through `rcrun mcp` is
  answered by the IDE: its Output shows the call, and the mascot sees it. With
  the IDE closed, a tool that needs it answers that PowerRustCOBOL AI must be
  open.
- [ ] **AC31** (R68) — While an agent is connected, a form changed on disk by a
  test (not by a tool) gets an Output line, and the next tool answer names the
  form and says it changed outside the MCP connection. A change written by a
  tool gets no such notice.
- [ ] **AC32** (R69) — The IDE theme and language changed by a tool are the
  ones Settings shows. The port and token are not offered.
- [ ] **AC33** (R70–R72) — Through tools, the IDE opens a form in a designer,
  selects a control, shows the Output panel and returns a picture of the
  window in which they show. Closing a tab that holds unsaved changes refuses
  and names them.
- [ ] **AC34** (R73) — The coverage test lists every menu entry, designer
  operation, Project Settings field and panel with its tool, and passes.

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
  for verification. *(Phase D: R70–R72 let the developer's Claude Code drive
  the developer's IDE through the token-gated MCP connection; that is a product
  capability. The operator's rule for agents that develop PowerRustCOBOL
  itself, "verify with tests, never by clicking the window", is unchanged.)*
- **Phase D:**
  - **i18n:** the Output lines of R68 and any new IDE text are translated in
    all six languages. Tool texts stay English (R29).
  - **Generated code:** only `regenerate` and the Build write `generated/`
    (R54).
  - **Docs:** the Guide's coding-agent section describes the tool groups, the
    MCP-only rule and R68.
  - **Fix vs feature:** feature; `z` only.
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

- **Q5 — The egui inspection endpoint (5719):** unchanged (operator,
  2026-11-18).
- **Q6 — A file changed outside the connection:** report it; do not refuse
  (R68; operator, 2026-11-18).
- **Q7 — Scope beyond the project:** IDE-wide settings, and the IDE's windows
  and state, which means driving the IDE's UI (R69–R72). Grace and the AI
  agents were not chosen (operator, 2026-11-18).
- **Q8 — Approval:** the operator asked for Phase D to be completed, merged
  into main and pushed while away, and to decide open points "as is right for
  the project in the long term" (2026-11-18). The decisions taken that way are
  recorded in `plan.md`, Phase D.

## 8. Open questions

- None outstanding. `/plan` must first confirm the Claude Code facts listed in
  section 6 against the installed version.
