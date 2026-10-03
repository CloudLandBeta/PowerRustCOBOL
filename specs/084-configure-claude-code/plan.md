<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Configure Claude Code (spec 084)

- **Status:** approved by the operator 2026-10-02
- **Spec:** ./spec.md   **Tasks:** ./tasks.md

## Context

Spec 080 exports a coding-agent kit into **each project** (CLAUDE.md, .claude/, .mcp.json,
a 740 KB copied reference pack). Any fix must be re-exported per project. The operator wants a
**one-time, IDE-wide** setup of their existing Claude Code: a plugin installed at user scope,
rules and knowledge served **live** by the IDE, Claude able to create/open/edit projects, an
IDE-wide **Claude Code Settings** window (Help menu), plus the follow-ups folded in:
`render_form`, `run_form`, application-level skills and a patterns pack.
Spec: `specs/084-configure-claude-code/spec.md` (R1–R38, AC1–AC20).

Work happens in worktree `.claude/worktrees/claude-config`, branch `feat/configure-claude-code`
(off main 1.80.100). One commit per phase step, each bumping `z` + CHANGELOG. First step copies
this plan into `specs/084-configure-claude-code/plan.md` and writes `tasks.md`.

## Facts established

- Claude Code 2.1.158 CLI has: `claude plugin marketplace add <path>`, `claude plugin install
  <p>@<mkt> --scope user --config key=value` (userConfig declared in the manifest),
  `claude plugin list --json`, `claude plugin uninstall|update|validate`,
  `claude mcp add --scope user --transport http <name> <url> -H "Authorization: Bearer …"`.
- Installed plugins on this machine use `${CLAUDE_PLUGIN_ROOT}` and `${user_config.<key>}`.
- Stdio MCP servers start with cwd = Claude's folder and get `CLAUDE_PROJECT_DIR` (tested).
- Port is already machine-wide: `LlmConfig.mcp_port` (`crates/cobolt-ide/src/llm.rs:81`, default
  5720, `resolve_mcp_port` :333), only *drawn* in `panels/settings_form.rs:1516`.
- IDE-wide settings pattern: `debug_settings.rs` (TOML in `llm::base_dir()`), Help menu
  `app.rs:15444`, Debug Settings item :15481, drawn :15292.
- Window pattern (no self-resize): `panels/leaderboard_modal.rs` (`size`, `fixed_size`, grip Area).
- Project create core: `create_new_project_at` (`app.rs:4353`); open: `open_project_at` (:4516);
  unsaved: `has_unsaved_changes()` (:11017). IDE has no project-path CLI arg (`main.rs:102`).
- Tool requests reach the UI thread via `HostRequest` (`agent_kit/ide_host.rs:62`) drained by
  `drain_agent_tools()` (`app.rs:4744`).
- External commands: model on `git_exec.rs:157 run_git` (captured output) and
  `toolchain.rs` (background thread + candidate path probing, :204–230, :363).
- Bundle: IDE and `rcrun` are siblings (`Contents/MacOS/` on macOS; flat elsewhere;
  `/opt/powerrustcobol` for .deb/.rpm). `project_model::find_cobolt_binary()` (:1691).
- Off-screen pixels: `resvg::tiny_skia` already in the tree (`cobolt-forms/src/paint.rs:7549`);
  `doc_shots.rs` captures only visible OS windows — not usable headless.
- MCP protocol lives in `crates/cobolt-mcp` (`McpHandler` server.rs:29, `dispatch` :86 shared by
  stdio and HTTP). It has **no** `instructions`, `resources/*`, image content or auth today
  (`InitializeResult` types.rs:135; `Content::Text` only, :160).
- HTTP transport `cobolt-project-tools/src/http.rs`: `/mcp/<kit_id>` gate (:179, :215), `Gated<H>`
  wrapper (:235) forwards only some methods. `ProjectTools::call` resolves the project **first**
  (tools/mod.rs:213) — project-free tools must dispatch before it. Only `list_files` names its project.
- `ProjectRoot::open` refuses an ancestor manifest (root.rs:39), so `rcrun mcp` in a sub-folder fails
  today; `cobolt_compiler::find_project_manifest` (lib.rs:1136) already walks upward.
- The assistant's KB is `cobolt_agents::chunked_knowledge::search` (chunked_knowledge.rs:647) over
  a redb store; `cobolt-agents` pulls rig/tokio/candle and is **not** linked by rcrun or
  project-tools. `kb_lookup` reads `cobolt_compiler::system_documentation()` (the same source text).
- Reference pack uses `docs_embed` (IDE-only `include_dir!`), so it cannot move to rcrun as is.
- The IDE's snapshot `unsaved` excludes `settings_dirty()`; `has_unsaved_changes()` includes it.
- Both listeners bind only at startup (port change needs restart today).

## Spec corrections to make in step 1

- R24/AC13: the port is already machine-wide (`LlmConfig.mcp_port`); the change is moving its row
  from the project Settings form to Claude Code Settings, and rebinding live (R25).
- R8: plugin-level tool pre-approval is subject to A0; if unsupported, a one-time "always allow"
  per tool is recorded as a deviation for the operator.

## Phase A — the integration

**A0 Spike (no product code; findings recorded in plan.md §Facts).** With a throwaway
`CLAUDE_CONFIG_DIR` (never the operator's config), build a minimal plugin + local marketplace and
confirm: plugin.json `userConfig` schema; `${user_config.x}` inside the plugin's `.mcp.json` for an
HTTP URL and a header; whether a plugin can pre-approve its MCP tools (settings in plugin) — if not,
the fallback is a one-time "always allow" prompt, recorded as a spec deviation for operator OK;
that server `instructions` and `resources/list` reach the model; `--config` re-run idempotence.
Decision rule: token/port via `userConfig` if it works, else `claude mcp add --scope user … -H`.

**A1 Shared content, reachable from rcrun.** Move the agent-neutral content (`agent_kit/content.rs`
rules, skills, reviewer, gap template; `reference.rs` builders `builtins_doc`/`cfrm_doc`/`cidx_doc`
and `KB_NAMES`) out of `cobolt-ide` into `cobolt-project-tools` (`src/content/`), which both the IDE
and `rcrun` link. The two docs the pack takes from `docs_embed` (Developer's Guide, supported
syntax) are embedded once for rcrun via `include_str!` in that module (or `crates/cobolt-docs` if
it already embeds them) — the source stays the single file in `docs/`. Rules drop their
`docs/powerrustcobol/` paths in favour of resource URIs. `redact.rs` gains a `(rel, text)` check
used over the plugin bundle.

**A2 MCP protocol, both servers.** In `cobolt-mcp` (one change, both front doors):
`McpHandler::instructions()` + `InitializeResult.instructions`; `list_resources`/`read_resource`
+ `resources/list|read` dispatch arms + `resources` capability; `Content::Image{data,mime_type}`.
`Gated` (http.rs) and `IdeTools` (ide_host.rs) forward the new methods. Then in
`cobolt-project-tools`:
- `instructions` = rules + version (R11); resources = reference pack (R13).
- `kb_search` (R12): lift the read side of `cobolt_agents::chunked_knowledge` — `ChunkRecord`,
  read-only open, the scoring + parent-chain reassembly of `search` (:647) and the
  `HashingEmbedder` (knowledge_store.rs:50) — into a dependency-light module in `cobolt-kb`;
  `chunked_knowledge` re-exports it so the assistant and both servers share one implementation.
  Source: the installed `~/PowerRustCOBOL/data/chunked.data` (the assistant's store) when present,
  else built in memory from `system_documentation()` with `chunk_markdown`. Hashing scoring by
  default (instant, no model); semantic via `cobolt_kb::semantic::BuiltinEmbedder` only when the
  model is already cached, loaded once per session. One `EXCLUDED_DOCUMENTS` constant (merge with
  `KB_LEFT_OUT`). Fix `kb_lookup`'s stale `docs/powerrustcobol/...` text.
- `create_project`, `open_project`, `kb_lookup`, `kb_search` dispatch **before** project
  resolution in `call` (tools/mod.rs:213).
- Every answer adds `project` (R19); optional `project` argument on every tool, refused on
  mismatch with both names (R20; reword `NoProject::Different`, host.rs:31).
- Extract the file-writing core of `create_new_project_at` (app.rs:4353: manifest, PROJECT_FOLDERS,
  starter main) into `project_model` so IDE and headless share it (R15, R18).
- rcrun reports the product version, not the crate version (main.rs:375) — share the constant.

**A3 IDE server.** `http.rs`: replace the `/mcp/<kit_id>` gate with a fixed `/mcp` path and a
bearer-token check (constant-time compare; 401 without it) (R35–R36); never log it (R38). Listener
can rebind when the port changes (R25). New `HostRequest::{CreateProject, OpenProject}` handled in
`drain_agent_tools` (app.rs:4744) → shared create core / `open_project_at`, refused while
`has_unsaved_changes()` (app.rs:11017, which includes the settings form) naming the files (R17).

**A4 rcrun.** `rcrun mcp` without `--project`: `CLAUDE_PROJECT_DIR` or cwd, search upward for
`*.project.toml` / `cobolt.toml` (R21). `open_project`/`create_project` with the IDE closed: launch
the sibling IDE binary with `--open <manifest>`, poll the IDE port until it answers (bounded),
report (R20a). IDE `main.rs`: parse `--open <path>` → `open_project_at` after startup.

**A5 Plugin bundle + Configure.** New `crates/cobolt-ide/src/claude_code/` (replaces
`agent_kit/`): `bundle.rs` writes the plugin from content into an IDE-owned folder
`llm::base_dir()/claude-code/marketplace/` (`.claude-plugin/marketplace.json`,
`powerrustcobol/.claude-plugin/plugin.json`, `skills/`, `agents/`, `.mcp.json` with the stdio
`rcrun mcp` server and, per A0, the HTTP server). Version = IDE version (R6). `configure.rs` runs
`claude plugin marketplace add|update`, `claude plugin install … --scope user --config …` on a
background thread through an injectable `CommandRunner` trait (real = `std::process::Command`,
test = recorder). `claude` discovery: PATH, `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin`,
`/usr/local/bin` (toolchain.rs pattern) (R3). Failure shows the command's stderr, leaves prior
config (R4). Redaction (`agent_kit/redact.rs`, kept) runs over every bundle file (R10).

**A6 Claude Code Settings window** (`panels/claude_code_settings.rs`): leaderboard window pattern
(R27); IDE-wide `ClaudeCodeSettings` TOML in `llm::base_dir()` holding the token; port stays in
`LlmConfig.mcp_port` but its row moves here from `settings_form.rs` (R24); shows Claude found/path,
plugin version (from `claude plugin list --json`, background), address; buttons Configure/Update,
Renew token (R23, R25, R26, R37). Help menu item beside Debug Settings.

**A7 Remove the per-project export.** File menu "Export coding-agent kit" → "Configure Claude Code"
(R1). Delete the per-project writer, stamp/KIT_MANIFEST/refresh-offer path (`agent_kit/stamp.rs`,
`app.rs:4619`, :12266, :12317) and its i18n keys; add new keys in all 6 languages (R29).

**A8 Docs.** Rewrite the guide's coding-agent section in `docs/developers-guide-en.md`, delete its
5 translations (GOLDEN RULE #8); CHANGELOG + version per commit.

## Phase B — seeing and running a form

Prereq from A2: `Content::Image` in `cobolt-mcp`.

No pixel path exists today (all goldens hash shapes). Decision: a **CPU rasteriser** of egui's
tessellated output — deterministic, no GPU, works on CI and headless.

**B1 Rasteriser** — `cobolt-forms::raster` (render feature): fill `ctx.tessellate` meshes
(barycentric, vertex colour × texture sample, premultiplied alpha, clip rects), applying
`textures_delta.set` (font atlas + images) instead of clearing it; `Shape::Callback` skipped;
PNG via `image` (already a dependency). Test: a coloured rect and text produce expected pixels.

**B2 `render_form`** (R30): lift `render_one` / `install_theme` / `form_backdrop` from
`cobolt-forms/tests/example_corpus_golden.rs` (:142–:193) into a library fn; two frames on a
headless `Context` with `base_font_definitions()`, rasterise, return `Content::Image`. Shell
option: refactor `run_shell` (shell.rs:1291) into `ShellApp::new` + a frame fn without
`eframe::Frame` (as `FormHost::ui_impl`), then `ensure_occupant`/`show_occupant`.

**B3 `run_form`** (R31): refactor `cmd_run_form` (`cobolt-cli/src/form_gui.rs:194`, uses
`process::exit`) into `prepare_form(cfrm, cbl, opts) -> Result<(FormHostConfig, …)>` shared by
`rcrun run-form` and a hidden `rcrun run-form-headless <cfrm> --script <json>` that prints a JSON
result. Add public `cobolt_form_host::headless` (`HeadlessRun`: step, send_event via
`FormBody::send_event`, set_prop via `StateUpdate` on state + input channels, `control_prop`
:4785, rgba). DISPLAY captured from the interpreter's `display_tx` (the host's own `println!`
gets a dummy receiver so stdout stays clean). Honour the 450 ms start hold. The MCP tool spawns
that subprocess (contains globals/panics, hard timeout + kill), injected into project-tools like
`build::Builder` (tools/build.rs:37) so project-tools gains no eframe/wgpu. Never opens a window.

## Phase C — building an application

**C1 Skills** (R32): build-an-application, shell-and-navigation, layout-and-themes in the shared
content; they name `render_form`/`run_form` for checks.
**C2 Patterns pack** (R33): curated forms from `examples/PowerDemo3` and `examples/PowerChat`
(shell, ContentPane form, indexed maintenance, REST call, AgentObject chat) embedded with
`include_str!` + a note each; served as resources; a test loads each and runs `check`.
**C3 Instructions** (R34): architecture section + resource/tool map.

## Verification

- Per step: `cargo test -p cobolt-project-tools`, `-p cobolt-cli`, `-p cobolt-form-host`,
  `-p cobolt-forms --features render`, `-p cobolt-ide --bin cobolt-ide --no-fail-fast`;
  read every `test result:` line (the known `docs_embed` translation red excepted).
- AC tests in Rust: recorder `CommandRunner` proves only `claude` commands run and no write to
  `~/.claude*` (AC2/AC3); in-process HTTP calls for token refusal, `instructions`, resources,
  `kb_search`, project mismatch (AC5–AC10, AC16); temp-folder `create_project` + `check` (AC8);
  `rcrun mcp` started in a sub-folder (AC11); `render_form` vs spec-056 golden (AC17);
  `run_form` button→label and infinite-loop timeout (AC18); patterns load+check (AC20);
  `i18n_tests` (AC14).
- End to end (manual, operator): Help → Claude Code Settings → Configure; in a project folder run
  `claude`, `/mcp` shows both servers; ask it to `kb_search` "SideMenu", create a project, add a
  form, `render_form` it. Not driven by the agent (CLAUDE.md: never drive the IDE).
