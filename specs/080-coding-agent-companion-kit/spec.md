<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Coding-agent companion kit (Claude Code first)

- **Status:** clarified (operator answered Q1–Q4, 2026-10-01)
- **Folder:** specs/080-coding-agent-companion-kit/
- **Author:** Anthropic Claude Codex Agent, for the operator   **Date:** 2026-10-01

## 1. Overview

PowerDemo3 and PowerChat were built by a coding agent (Claude Code) working
with full access to the PowerRustCOBOL source. A developer using the
**PowerRustCOBOL AI binary** has nothing comparable. Their agent cannot see
the compiler, does not know which verbs, controls, properties and events
exist, and does not know that generated COBOL is never hand-edited. It
guesses, and the guesses do not compile.

This feature exports a **companion kit** into the developer's project. The
kit gives an external coding agent four things:
- the rules and the reference it needs to write correct COBOL, forms
  (`.cfrm`), indexed-file definitions (`.cidx`) and assets for a **desktop**
  PowerRustCOBOL project;
- permissions that keep it inside that project;
- a connection to the running IDE, so it can check and build its own work;
- a fixed way to report what the compiler or IDE would need for a request it
  cannot fulfil today.

The first target is Claude Code. The kit generator is agent-neutral, so other
coding agents can be added later as further targets.

## 2. Goals / Non-goals

**Goals**
- One IDE action writes a kit into the open project. With it, Claude Code can
  create and update that project's COBOL, forms, indexed files and assets at
  the quality seen in PowerDemo3 and PowerChat: it compiles, and it follows
  the generated-code contract.
- The kit's reference matches **the binary that wrote it** (the same version):
  - verbs and the supported COBOL-85 syntax;
  - built-ins, written inline as `COBOL::"…"`;
  - controls with their properties, methods and events;
  - the `.cfrm` and `.cidx` formats;
  - the Guide.
- Through the IDE's MCP server, the agent can check, regenerate and build, and
  read the diagnostics, before it calls a change done.
- The agent never modifies PowerRustCOBOL itself. When a request needs an
  unsupported capability, it writes a Markdown gap report into the project,
  does the supported part, and says so.
- The generator is target-neutral: the content model is shared and only the
  writer is per-agent. Claude Code is the one writer in this spec.

**Non-goals**
- Mobile, Web/WASM or cloud targets. Scope is the project types that exist
  today (desktop).
- Driving the IDE's UI on the agent's behalf. The MCP tools are
  project-level operations, not clicks.
- Writers for any agent other than Claude Code.
- Shipping or embedding any secret. The kit carries no API key, consistent
  with 1.80.30's AI export.
- Changing the compiler to close a gap the agent reports. Reports are input
  to the operator; they are not acted on automatically.

## 3. User stories

- As a COBOL developer using the PowerRustCOBOL AI binary, I want to press
  one button and have Claude Code understand my project, so that it builds
  forms and programs that compile the first time.
- As that developer, I want Claude Code to check its own work against the
  real compiler, so that I do not get code that only looks right.
- As that developer, when I ask for something the product cannot do, I want
  a clear written report of what is missing instead of invented syntax, so
  that I can send it to the PowerRustCOBOL team.
- As the operator, I want those reports in one consistent format, so that
  they can be triaged as fixes or features.
- As the operator, I want the kit to support other coding agents later
  without rewriting its content.

## 4. Requirements (EARS)

**Export**
- **R1 (event):** When the developer chooses **Export coding-agent kit…** with
  a project open, the IDE shall write the kit for the chosen target (only
  "Claude Code" in this spec) into the project folder. It shall then list in
  the Output panel every file it wrote.
- **R2 (constraint):** The kit for Claude Code shall consist of:
  - `CLAUDE.md`, the agent brief;
  - `.claude/settings.json`, holding the permissions;
  - `.claude/skills/*`, the task skills;
  - `.claude/agents/*`, a reviewer subagent;
  - `.mcp.json`, the IDE connection;
  - the reference pack, under `docs/powerrustcobol/`.
- **R3 (constraint):** The export shall not overwrite anything the developer
  wrote:
  - Kit-owned files carry a version stamp and are replaced on re-export.
  - Where the developer already has a `CLAUDE.md`, the kit owns only a
    delimited section of it. Everything outside that section is preserved
    byte for byte.
  - A kit-owned file the developer has edited (its stamp no longer matches
    its content) is not overwritten. It is named in the Output panel instead.
- **R4 (constraint):** The kit shall carry no secret and nothing personal
  about the developer:
  - no API key, token, password or credential of any kind;
  - not the developer's home-folder path, login name, or git name and
    e-mail. These are replaced by placeholders, as in the AI export
    (1.80.32).
  The export is refused if any key stored on the machine still appears in
  any kit file, or if any of those personal details still appears in a file
  the export **generates** or in a value it inserts (a path, a name). The
  reference files copied verbatim (the Guide, the syntax document) are
  checked for stored keys only: a login such as `main` or `data` would
  otherwise match ordinary words in them (operator, 2026-10-01, ruling on
  plan finding F1).
- **R5 (ubiquitous):** Every kit file shall record the IDE version that wrote
  it. A Markdown file records it in a comment at its top. A JSON file
  (`.claude/settings.json`, `.mcp.json`) cannot hold a comment, so its version
  and content stamp are recorded in the kit manifest,
  `.claude/powerrustcobol-kit.json`, which lists every kit file with the
  version that wrote it and a hash of what was written (the stamp R3 compares).
  *(clarified 2026-10-01)*
- **R6 (state):** While the kit's version differs from the running IDE's, the
  IDE shall say so when the project opens and offer to refresh the kit.

**Reference pack (what makes the code right)**
- **R7 (constraint):** The reference pack shall be generated from the same
  sources the binary uses, never hand-maintained. It contains the **whole**
  English Developer's Guide plus the generated tables (operator, Q4,
  2026-10-01); relevance is left to the agent's own search and to the KB
  lookup tool (R11). Its sources:
  - the System KB control, property, method and event tables;
  - the built-ins registry (`BUILTINS`);
  - the supported-syntax document;
  - the `.cfrm` and `.cidx` format descriptions;
  - the English Developer's Guide.
  A property that is not in the pack does not exist for the agent.
- **R8 (ubiquitous):** The brief and the skills shall state the project's
  standing COBOL rules as instructions to the agent:
  - Generated `.cbl` is never edited; the agent edits the `.cfrm` and
    regenerates.
  - Built-ins are written inline as `COBOL::"NAME" ( … )`.
  - COBOL identifiers and source stay in English.
  - Event handlers fire only when they are bound.
  - Exactly one main form.
  - A container is defined by its `parent` field, not by geometry.
  - A runtime property is only one the reference lists.
- **R9 (ubiquitous):** The kit shall include skills for at least:
  - creating a form;
  - adding a control and binding its event;
  - defining an indexed file and its generated facade;
  - adding assets;
  - writing a common procedure;
  - the check-and-fix loop;
  - writing a gap report.
- **R10 (ubiquitous):** The kit shall include a reviewer subagent. It checks
  a change against the reference (no invented verb, property or event) and
  against the rules in R8 before the change is reported done.

**MCP tools (how the agent checks its work)**
- **R11 (state):** While a project is open, the IDE shall serve MCP tools
  that act on that project only. The same tools shall also be served headless
  by **`rcrun mcp`**, so the agent can check and build with the IDE closed
  (operator, Q2, 2026-10-01). Both servers dispatch one shared implementation
  of the tools; neither is a special case of the other. The tools:
  - list the project's files;
  - check (diagnostics, with file and line);
  - regenerate the COBOL of one form or of all forms;
  - build the binary;
  - validate a `.cfrm` or `.cidx` file;
  - look up a control, property, method, event or built-in in the System KB;
  - add a new form or indexed-file definition to the project, re-sealing the
    main-form designation the way the IDE does, so the manifest is never
    hand-edited (operator, 2026-10-01, ruling on plan finding F5).
- **R11a (ubiquitous):** The IDE serves the tools over HTTP on `127.0.0.1`, on a
  port that is an IDE setting with a fixed default; the export writes that
  port into `.mcp.json`, so changing it means re-exporting. `rcrun mcp
  [--project <manifest>]` serves them over stdio (one JSON-RPC message per
  line), started by the agent itself as an MCP command; without `--project`
  it uses the project manifest in its working directory. `.mcp.json` names
  both servers. *(port and manifest defaults clarified 2026-10-01)*
- **R12 (constraint):** The MCP tools shall be reachable from the same machine
  only: the IDE server is bound to `127.0.0.1`, and `rcrun mcp` uses stdio and
  opens no port (operator, Q1, 2026-10-01: a remote or web Claude Code is a
  later spec). The tools shall refuse any path outside the project, and
  shall not write outside it.
- **R13 (constraint):** The MCP tools shall not expose any operation on the
  PowerRustCOBOL installation, the machine configuration or API keys.
- **R14 (event):** When a tool is called while no project is open, or while
  the open project is not the one the kit was written for, the tool shall
  answer with that fact. It shall not act on another project. For `rcrun mcp`
  the project is the one it was started with; a manifest that cannot be read
  makes every tool answer "no project".

**Restriction**
- **R15 (constraint):** The permissions in `.claude/settings.json` shall:
  - allow edits only inside the project;
  - allow the IDE's MCP tools;
  - deny shell commands, except read-only listing and searching inside the
    project.
- **R16 (constraint):** The brief shall forbid the agent to modify, or to ask
  for access to, the PowerRustCOBOL product itself (compiler, runtime, IDE).

**Gap reports**
- **R17 (event):** When a request needs a verb, control, property, method,
  event, file feature or IDE capability that the reference does not list, the
  agent shall:
  - write `docs/compiler-requests/<YYYY-MM-DD>-<topic>.md` in the project;
  - implement whatever part of the request is supported;
  - tell the developer both.
- **R18 (ubiquitous):** A gap report shall contain:
  - the developer's request;
  - the missing capability, named precisely;
  - why it is needed (what cannot be done without it);
  - a minimal COBOL or form example of the wanted behaviour;
  - the workaround used, if any;
  - the kit and IDE version;
  - a classification as COBOL-85 standard behaviour (a fix) or a capability
    beyond it (a feature).
- **R19 (constraint):** The agent shall not invent syntax, properties or
  events to avoid writing a report.
- **R19a (state):** While a project has files in `docs/compiler-requests/`,
  the IDE project tree shall show a **Compiler requests** node listing them,
  newest first, each opening in the editor (operator, Q3, 2026-10-01). The
  node is read from the folder, so a report written by the agent appears
  without any other step; the node is absent when the folder is empty.

**Agent-neutral generation**
- **R20 (ubiquitous):** The kit's content (rules, reference, skills, report
  template) shall be produced once, target-independently. A per-target writer
  turns it into that agent's files. Adding a target shall not require
  changing the content.

**IDE**
- **R21 (ubiquitous):** Every new IDE string shall be translated in all six
  languages.
- **R22 (constraint):** The kit's own text (brief, skills, reference) shall be
  English. Generated COBOL in any example shall be English.

## 5. Acceptance criteria

- [ ] **AC1 (R1, R2, R5)** — Exporting into a copy of PowerChat writes every
  file listed in R2. Each carries the IDE version, and the Output panel lists
  them.
- [ ] **AC2 (R3)** — A project with its own `CLAUDE.md`, and with one edited
  kit skill, keeps both. The developer's text survives byte for byte, the
  edited skill is not overwritten, and the skill is named in the Output
  panel.
- [ ] **AC3 (R4)** — With a key stored on the machine, no kit file contains it,
  nor the developer's home path, login, git name or e-mail, even when they
  are planted in a prompt or a path. A planted key or detail that cannot be
  replaced makes the export refuse, and nothing is written.
- [ ] **AC4 (R7)** — Every control type, property, method and event in the
  System KB tables, and every `BUILTINS` entry, appears in the reference
  pack. A test compares the two sets.
- [ ] **AC5 (R11, R12, R14)** — An MCP client lists the tools and runs check
  on a project with an error, getting the file and line. A call with
  `../` or an absolute path outside the project is refused. With no project
  open, every tool answers "no project open".
- [ ] **AC6 (R11)** — Regenerating a form through MCP produces the same
  `.cbl` as the IDE's Generate. Building through MCP produces the binary.
- [ ] **AC6a (R11, R11a, R12)** — `rcrun mcp --project <manifest>` answers
  `initialize` and `tools/list` over stdio with the same tool set as the IDE
  server, runs check with the same result, and opens no network port.
- [ ] **AC6b (R19a)** — With two reports in `docs/compiler-requests/`, the
  project tree shows a Compiler requests node with both, newest first; with
  none, no node.
- [ ] **AC7 (R15)** — The written `.claude/settings.json` parses. It contains
  the allow and deny rules of R15, and no rule that grants access outside the
  project.
- [ ] **AC8 (R17, R18)** — The gap-report skill's template contains every field
  in R18.
- [ ] **AC9 (R20)** — The content model is serialised once. A test target
  writer (test-only) renders the same content to a different layout without
  touching the content code.
- [ ] **AC10 (end to end, operator-run)** — In a fresh desktop project with the
  kit, Claude Code is asked for:
  - a record-entry form over a new indexed file;
  - one capability the product lacks.
  It delivers a form that passes MCP check and runs, and a gap report for
  the missing piece. It edits nothing outside the project.
- [ ] **AC11 (R21)** — The new `Tr` keys exist in all six tables.

## 6. Constraints & steering check

- **i18n:** yes. Menu entries, Output messages, and the refresh offer (R6)
  need all six languages. The kit's content stays English (R22).
- **Generated-code contract:** unchanged. The kit teaches it, and the MCP
  regenerate tool uses the same generator as the IDE (AC6).
- **System KB:** the reference pack is generated from the KB tables, so no
  table changes. If the MCP KB lookup tool (R11) is documented as a
  capability, it is documented in the Guide, not in the control tables.
- **Docs:** a Guide section, "Working with a coding agent". The translations
  of the Guide are deleted per GOLDEN RULE #8 (only the English one exists
  today).
- **Security:**
  - The MCP tools listen on localhost only, on the same model as the
    inspection endpoint.
  - No authentication. Anything on the machine that can reach the port can
    call them, and the Guide must say so.
  - No secrets in the kit (R4).
- **Fix vs feature:** a **feature**, a new capability beyond the IDE's scope.
  It goes on `features`, with a z bump per change.
- **Pairing with 1.80.30:** the AI configuration export (models, agents,
  leaderboard) and this kit are separate files with separate purposes. The
  kit does not include the AI export.

## 7. Decisions (were open questions)

Answered by the operator on 2026-10-01:

- **Q1 — Remote Claude Code.** Same machine only. The IDE server listens on
  `127.0.0.1`; `rcrun mcp` is stdio. A remote or web Claude Code needs an
  authenticated network endpoint and is a later spec. → R12.
- **Q2 — Without the IDE running.** Yes: `rcrun mcp` serves the same tools
  headless. → R11, R11a, AC6a.
- **Q3 — Where the gap report goes.** A file in the project **and** a
  Compiler requests node in the IDE project tree. → R17, R19a, AC6b.
- **Q4 — Size of the reference pack.** The whole English Guide plus the
  KB-generated tables. → R7.

Rulings on the plan's findings, 2026-10-01: **F1** — R4 checks stored keys in
every kit file and personal details in generated files and inserted values
only (→ R4). **F5** — the tools include adding a form or indexed file to the
project (→ R11).

Assumptions recorded during clarification (not operator decisions; `/plan`
may revisit them):
- A JSON kit file's version and stamp live in the kit manifest (R5).
- The IDE server's port is an IDE setting with a fixed default, baked into
  `.mcp.json` at export (R11a).
