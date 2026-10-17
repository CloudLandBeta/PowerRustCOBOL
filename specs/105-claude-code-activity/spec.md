# Spec — Claude Code Activity

- **Status:** draft — waiting for the operator's review and `/clarify` (the operator asked, 2026-10-17, for the spec first; nothing here is implemented)
- **Folder:** specs/105-claude-code-activity/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-17

## 1. Overview

The operator's words: *"I also would like the IDE/RAD show the actions performed by Claude
Code when implementing a project so I can see what it is doing."*

When Claude Code builds or changes a PowerRustCOBOL project, the developer watching the IDE
sees almost nothing. Today the IDE shows **one Output line per call to its own tool server**
("Coding agent: regenerate forms/Main.cfrm"). That line is written *before* the call runs, so
it carries no result. It is written even for a call the IDE then refuses. The next Run or
Check wipes it. And it covers only the IDE's own tools: everything Claude Code does with its
**own** tools is invisible — reading a file, editing a form's XML, writing a COBOL program,
running a shell command. That is most of what it does.

This feature makes Claude Code's work **visible while it happens**, in two places:

- **In the IDE:** an **Activity** view. It lists what Claude Code is doing, live, grouped by
  the request it is working on, with each action's outcome. It also shows a one-line live
  status the developer can see from anywhere in the IDE.
- **In the RAD (the Form Designer):** a form Claude Code is changing **shows the change**. An
  open designer follows the file on disk, the controls that moved, appeared or changed are
  visibly marked, and the designer says that Claude Code did it. Today a clean designer is
  silently replaced — undo, selection and zoom are lost — and only when one of the IDE's own
  tools happens to be called. A dirty designer is silently skipped, so the developer's next
  save overwrites Claude Code's edit without a word.

## 2. Goals / Non-goals

**Goals**

- Every action Claude Code takes on the open project shows up in the IDE **within about a
  second**, with its outcome: done, failed, or refused. That covers the IDE's own tools, its
  command-line tool server and Claude Code's own file and shell tools.
- Actions are grouped under the request (turn) that caused them, and the request's start and
  end are visible: the developer can tell "working" from "finished" from "waiting for you".
- The Form Designer follows Claude Code's changes to an open form **without losing the
  developer's undo history**, shows what changed, and never silently discards anyone's
  edit — the developer's or Claude Code's.
- Nothing the feed shows or keeps contains a secret.
- It never slows Claude Code down, and nothing breaks when the IDE is closed.

**Non-goals**

- **Controlling Claude Code from the IDE.** No approve/deny buttons, no stopping a turn, no
  sending prompts. The feed only watches.
- **A transcript viewer.** It shows *actions*, not Claude's prose or reasoning, and never
  reads Claude Code's transcript file.
- **Other coding agents.** The mechanism is Claude Code's (its plugin, its hooks). Another
  agent that calls the IDE's tool server still appears for those calls, and nothing more.
- **Grace.** Grace's live action status (spec 036) stays as it is; the two are not merged.
- **Replaying or undoing Claude Code's work as a unit.** Out of scope; each designer change
  stays ordinary undo.

## 3. What exists today (the facts this spec stands on)

| Path | What it is | What the IDE learns today |
|---|---|---|
| IDE tool server (`powerrustcobol-ide`, HTTP on `127.0.0.1:<port>`, bearer token) | `crates/cobolt-project-tools/src/http.rs`, wrapped by `agent_kit/ide_host.rs` `IdeTools` | One `HostRequest::Called{tool,target}` per call, **before** it runs → one Output `Status` line (`tr.mcp_activity`). No result, no duration, logged even when `decide()` refuses; calls refused for "no project open" are not logged at all. |
| Command-line tool server (`powerrustcobol`, `rcrun mcp`, stdio) | `cobolt-cli/src/main.rs` `cmd_mcp`, `HeadlessHost` | **Nothing.** It knows the IDE's port (`machine_mcp_port()`) but does not report to it. |
| Claude Code's own tools (Read, Edit, Write, Bash, Grep, Glob, WebFetch…) | Claude Code itself | **Nothing.** The plugin the IDE ships (`claude_code/bundle.rs`) has skills, an agent and `.mcp.json`, and **no hooks**. |
| Files changed on disk | — | No file watcher. Clean designers reload only when an IDE tool call arrives (`reload_forms_changed_on_disk`), as a whole replacement (`DesignerPanel::new`): undo, selection and zoom are lost and nothing is animated. Dirty designers are skipped silently. Editor tabs reload only on `HostRequest::Written` (regenerate's outputs); a `src/*.cbl` or `.cidx` edited by Claude's own tools is never reloaded. |
| Who is calling | — | Not captured. `initialize`'s `clientInfo` is ignored; `User-Agent` is not read. |

Claude Code's plugins can ship **hooks** (`hooks/hooks.json` in the plugin). Verified against
the Claude Code hooks and plugin references on 2026-10-17:

- The events are PreToolUse/PostToolUse/PostToolUseFailure per tool call (built-in **and** MCP
  tools), UserPromptSubmit/Stop/StopFailure per turn, SubagentStart/SubagentStop and
  Notification.
- Every payload carries `session_id`, `cwd` and `hook_event_name`; tool events add
  `tool_name`, `tool_input` and `tool_use_id`, and PostToolUse adds `tool_response` and
  `duration_ms`.
- A `command` hook can run **asynchronously**: Claude does not wait, and its outcome cannot
  block anything.
- A failing PostToolUse hook never blocks the tool.

Unverified, and to be probed in `/plan`: `MultiEdit`'s name and payload, `tool_response`'s
shape for Edit/Read/Grep, how a plugin's hooks behave before the folder is trusted, and the
`clientInfo` values Claude Code sends.

Prior art to reuse:

- `agent_actions.rs` (Grace's closed `ActionKind` set and `ActionThrottle`);
- spec 035's `apply_agent_change_set`, which plays the glide and spawn animations for an
  in-IDE agent change set;
- the per-designer warning strip (`Panel::top("dwarn_{idx}")`);
- spec 084 R38, which forbids the token in any log.

## 4. User stories

- As a developer who asked Claude Code to build a screen, I want to see which files it is
  reading and changing, right now, so I know it is working on the right thing and can stop
  it early if it is not.
- As a developer with the form open in the Form Designer, I want to watch the controls appear
  and move as Claude Code edits the form, so I see the result take shape without reopening
  anything.
- As a developer who was editing the same form, I want to be told that Claude Code changed it,
  and to choose, so that neither my edit nor Claude Code's is lost behind my back.
- As a developer coming back to the screen, I want to see whether Claude Code is still
  working, finished, or waiting for my answer, without switching to the terminal.
- As the operator, I want certainty that no API key, token or password ever lands in the feed
  or a log.

## 5. Requirements (EARS)

### What is captured

- **R1 (event):** When Claude Code calls one of the IDE's own tools, the feed shall record the
  call when it **starts** and update the same entry when it **ends**: done, failed (with the
  tool's own message), or refused (with the reason the IDE gave, such as "unsaved form"). A
  call refused because no project is open shall be recorded too.
- **R2 (event):** When Claude Code calls the command-line tool server (`rcrun mcp`) while the
  IDE is running, the IDE shall record the call as for R1. When the IDE is not running, the
  server shall behave exactly as it does today.
- **R3 (event):** When Claude Code uses one of its own tools on the open project — reading,
  searching, editing or writing a file, running a shell command, fetching a page — the IDE
  shall record it, with its outcome, through **hooks shipped in the PowerRustCOBOL plugin**.
- **R4 (event):** When a request (turn) starts, ends, ends in an error, or stops to wait for
  the developer (a permission prompt, a question), the feed shall show it.
- **R5 (ubiquitous):** Each entry shall carry: when it happened, the session it belongs to, the
  kind of action from a **closed set** (read, search, edit, write, run command, fetch, IDE
  tool, turn start, turn end, waiting), the item it touched, and its outcome and duration
  when it has ended.
- **R6 (constraint):** An item outside the open project's folder shall be shown by its file
  name only, never its full path. Claude Code's actions in another project shall not appear.
- **R7 (ubiquitous):** Entries shall be attributed to their **session**. When two Claude Code
  sessions work on the same project, the feed shall keep them apart.
- **R8 (constraint):** The capture shall never make Claude Code wait. A hook shall be
  asynchronous or answer within a bound measured in milliseconds. A hook that cannot reach
  the IDE (closed, another port, another machine) shall end successfully and silently. No
  hook shall ever block, deny or alter a tool call.
- **R9 (constraint):** Only Claude Code on this machine, holding the IDE's access token, shall
  be able to add entries. The feed's endpoint shall listen on loopback only, require the same
  bearer token as the tool server, cap the size of a report, and treat every report as
  untrusted data: shown as text, never executed, never followed as a link outside the
  project.

### Secrets

- **R10 (constraint):** The feed, its live status, any log it writes and any file it saves
  shall **never contain**:
  - the IDE's access token;
  - any file's contents;
  - an edit's old or new text;
  - a form property's value (example forms carry live keys);
  - a command's output;
  - a fetched page.
- **R11 (constraint):** A shell command shall be shown by the **description** Claude Code gives
  it. Its command line shall appear only after redaction of every recognisable secret
  (key-shaped tokens, `…KEY=`, `…TOKEN=`, `…PASSWORD=`, `Authorization:` values, URL
  credentials); a command that cannot be redacted with confidence shall show its description
  alone.

### In the IDE

- **R12 (ubiquitous):** The IDE shall have an **Activity** view listing the entries newest
  first, grouped under their request, with the request's state (working / finished / failed /
  waiting for you) on its header.
- **R13 (state):** While a Claude Code session is working on the open project, the IDE shall
  show a **live status** visible from anywhere in the IDE, naming the current action ("Claude
  Code — editing forms/Customers.cfrm"). It shall change at most once a second, as Grace's
  does, and say *finished* or *waiting for you* when the turn ends or stops.
- **R14 (event):** When the developer activates an entry for an item in the project, the IDE
  shall open it where it lives: a form in its designer, a COBOL file in the editor, an
  indexed file in its editor. An entry for anything else shall open nothing.
- **R15 (ubiquitous):** Entries shall survive Run, Check and Clear of the Output pane. They
  shall be kept for the IDE session up to a bounded number, oldest dropped first, and can be
  copied or saved as text the same way the Output pane's log can.
- **R16 (optional):** Where the developer turns the feed off (Claude Code Settings), the IDE
  shall record nothing, show no status, and the hooks shall still end silently.

### In the RAD (Form Designer)

- **R17 (event):** When a form open in a **clean** designer changes on disk — by Claude Code or
  by anything else — the designer shall load the new version within about a second, as **one
  undo entry**, keeping selection (for controls that still exist), zoom and scroll. It shall
  not wait for an IDE tool call.
- **R18 (event):** When that reload changes controls, the designer shall show **which ones**:
  added controls appear, moved or resized ones glide to their new place (spec 035's
  animations), changed ones are marked briefly. The designer shall name the source ("Claude
  Code changed this form") when the feed attributes the write to Claude Code.
- **R19 (event):** When a form open in a **dirty** designer changes on disk, the designer shall
  say so in its warning strip, offering **Load theirs** (the developer's unsaved changes go
  to the undo history, so nothing is lost) and **Keep mine** (the next save is the
  developer's deliberate choice). Until the developer chooses, Save shall ask before
  overwriting. *(User code is sacred, in both directions.)*
- **R20 (event):** When a COBOL source, copybook or indexed definition open in an editor
  changes on disk, the same rule shall apply: a clean tab reloads, keeping its caret and
  scroll where possible; a dirty tab offers the choice of R19.
- **R21 (state):** While Claude Code is changing a form that is open in a designer, that
  designer shall show, in its own window, that Claude Code is working on it and its latest
  action.

### Fit with the IDE

- **R22 (ubiquitous):** Every string — the view, the status, the kinds, the outcomes, the
  strip, the buttons — shall be a `Tr` field in all six languages.
- **R23 (constraint):** No window shall resize itself; the Activity view follows the IDE's
  explicit-size rules.
- **R24 (constraint):** The file-change detection shall add no dependency the workspace does not
  already have, shall cost nothing measurable while no project is open, and shall work the
  same on macOS, Linux and Windows.
- **R25 (ubiquitous):** Configuring Claude Code (spec 084) shall install the hooks with the
  rest of the plugin. A plugin installed by an older IDE shall keep working without them,
  and the Claude Code Settings window shall say that reinstalling adds the activity feed.
- **R26 (constraint):** The hooks shall be written in the plugin as configuration that calls
  PowerRustCOBOL's own Rust binary, or as Claude Code's own declarative HTTP hook — never as
  a shell, Python or other script (the repository's prime directive).

## 6. Acceptance criteria

- [ ] AC1 — A call to an IDE tool produces one entry that starts as *running* and ends as
  *done*, *failed* (with the tool's message) or *refused* (with the reason); a call with no
  project open is recorded as refused (R1).
- [ ] AC2 — The same holds for a call to `rcrun mcp` while the IDE runs; with the IDE closed
  `rcrun mcp` answers byte-for-byte as before (R2).
- [ ] AC3 — A fixture of hook payloads (Read, Edit, Write, Bash, Grep, an MCP tool, UserPromptSubmit,
  Stop, Notification) fed to the endpoint yields the expected entries, kinds, outcomes and
  grouping (R3–R5, R7).
- [ ] AC4 — A hook run with the IDE closed exits 0 with no output, in under the bound set in
  `/plan`; with a wrong token the endpoint answers 401 and records nothing (R8, R9).
- [ ] AC5 — A payload whose tool input, tool response, edit text, command and form XML carry a
  planted `sk-TEST-…` key, `ANTHROPIC_API_KEY=…`, a bearer header and a URL with credentials
  produces entries, a status, a saved log and a `Debug` dump in which **none** of them
  appears (R10, R11).
- [ ] AC6 — A path outside the project shows as its file name; an action in another project
  produces no entry (R6).
- [ ] AC7 — The live status changes at most once per second under a burst of 100 events, and
  shows *finished* / *waiting for you* on Stop / Notification (R13).
- [ ] AC8 — Activating a form, COBOL and indexed entry opens each in its own editor; an
  outside entry opens nothing (R14).
- [ ] AC9 — Run, Check and Clear leave the feed intact; the buffer is bounded (R15).
- [ ] AC10 — With the feed off, the endpoint records nothing and the hooks still exit 0 (R16).
- [ ] AC11 — A clean designer whose `.cfrm` is rewritten on disk with one control added, one
  moved and one property changed reloads within a second as one undo entry: undo restores the
  previous form; selection, zoom and scroll are kept; the added and moved controls animate
  (R17, R18).
- [ ] AC12 — A dirty designer whose file changes on disk shows the strip; *Load theirs* keeps
  the developer's version reachable by undo; *Keep mine* followed by Save writes the
  developer's version only after the confirmation (R19).
- [ ] AC13 — The same two cases pass for an editor tab (R20).
- [ ] AC14 — Every new `Tr` field is present in six languages and the `ui_text` source scans
  stay green (R22).
- [ ] AC15 — The generated plugin contains the hooks file; its hooks name only the IDE's own
  binary or a declarative HTTP hook; no script file is in the bundle (R25, R26).

## 7. Constraints & steering check

- **i18n:** every string is a `Tr` field in EN/ES/PT/JA/ZH/FR (R22).
- **Generated code:** none. The `.cfrm`, `.cidx` and generated COBOL formats are untouched.
- **Plugin bundle:** changes (a hooks file), so the plugin version moves with the IDE as
  today. Spec 084's R9/R10 (no `CLAUDE.md`, no token or home folder in any file) still hold.
- **Docs:** the Developer's Guide (English) gets an *Activity* section and an update to the
  *Claude Code* chapter; per GOLDEN RULE #8 the English canonical only.
- **System KB:** no behaviour of the language, controls or runtime changes, so no KB update,
  unless `/plan` puts a new tool on the server.
- **Fix vs feature:** a **feature** (a new IDE capability), with one embedded **fix**: R19's
  silent overwrite of Claude Code's edit by a dirty designer's save is a defect today. It can
  ship on the fixes branch ahead of the feature if the operator prefers (Q4).
- **Dependencies:** no new crate. File-change detection polls modification times, as the
  project tree and the Main-Pane inspector already do.

## 8. Risks

- **Process cost.** An asynchronous `command` hook starts one process per tool call. A burst of
  Read/Grep calls starts a burst of processes. `/plan` must measure the IDE binary's
  start-up. If it is too slow, the cheaper alternatives are:
  - a small dedicated subcommand;
  - Claude Code's declarative `http` hook with a short timeout. It is synchronous, so the
    timeout is the cost when the IDE is closed. Whether a failed `http` hook prints a "hook
    error" line in the transcript must be checked; R8 forbids noise.
- **Workspace trust.** How a plugin's hooks behave before the folder is trusted is not
  documented. If they wait for trust, the feed starts after trust, and the Settings window
  must say so.
- **Reload as one undo entry.** Today's whole-replacement path has no undo. R17 needs a
  diff-to-commands path. Spec 035's `apply_agent_change_set` is the template, but it was
  built for in-IDE change sets and must be checked against arbitrary on-disk edits (layers,
  handlers, structure blocks).
- **Two writers.** Claude Code may write a form while the developer drags a control in it.
  R19 covers the dirty case. The window between "clean" and "dirty" inside one frame must be
  closed in `/plan`.

## 9. Open questions (for `/clarify`)

- **Q1 — Where does the Activity view live?**
  - (a) A tab beside Output in the bottom pane. Output has no tabs today, so this adds them.
  - (b) A side pane that can be shown and hidden from the View menu.
  - (c) A window opened from the live status.

  The author's recommendation is (a), with the live status in the toolbar's right group.
- **Q2 — How long is the feed kept?**
  - (a) For the IDE session only.
  - (b) Per project, kept on disk across restarts. It would be machine-level and never in the
    project folder.

  Recommendation: (a), plus Save as text.
- **Q3 — What is shown for a shell command?**
  - (a) Claude's description, plus the redacted command line.
  - (b) The description only.

  Recommendation: (a), under R11's rule.
- **Q4 — Should R19 (the silent overwrite) ship first as a fix,** on the fixes branch, before
  the feature?

  Recommendation: yes. It is a data-loss defect today, whether or not anyone watches the
  feed.
- **Q5 — On by default?** Recommendation: on. It costs nothing when Claude Code is not running,
  and R16 turns it off.
