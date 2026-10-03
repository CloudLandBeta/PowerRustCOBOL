<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Configure Claude Code (spec 084)

- **Status:** approved (operator, 2026-10-02) — `/implement` in progress
- **Plan:** ./plan.md   **Branch:** `feat/configure-claude-code` (worktree
  `.claude/worktrees/claude-config`), landed on `main` only when the operator asks.

## Standing rules for every task

- **One task = one commit.** Each bumps `z` in `crates/cobolt-ide/src/version.rs`
  and adds a dated top-of-file `CHANGELOG.md` entry. Feature commits only; a
  fix found on the way goes to its own `fix/…` branch from `main`.
- **Every new test prints a quantified summary** (GOLDEN RULE #7).
- **Sweeps** use `--no-fail-fast`; read every `test result:` line. `cobolt-ide`
  tests run with `--bin cobolt-ide`. The `docs_embed` translation guard is a
  known red until the next minor regenerates translations.
- **Never drive the IDE** to verify. Tests and builds only; the operator looks.
- **Spikes never touch the operator's Claude Code config**: a throwaway
  `CLAUDE_CONFIG_DIR` in the session scratchpad, deleted afterwards.

## Phase A — the integration

- [x] **T0 — Spec, plan, tasks.** Record the approved plan and these tasks;
  apply the R8/R24/AC13 corrections. *Verify:* files present.
- [x] **T1 (A0) — Claude Code spike.** Minimal plugin + local marketplace under a
  throwaway `CLAUDE_CONFIG_DIR`: `userConfig` schema and `--config`;
  `${user_config.x}` in the plugin's `.mcp.json` (URL and header); plugin-level
  tool pre-approval; server `instructions` and `resources/list` reaching the
  model; re-install idempotence. Findings into plan.md §Facts; decide
  userConfig vs `claude mcp add -H`. *Serves:* R2, R6–R8, R11, R13, R35.
- [x] **T2 (A2a) — `cobolt-mcp`: instructions, resources, image content.**
  `McpHandler::instructions`, `list_resources`/`read_resource`, dispatch arms,
  `resources` capability, `Content::Image`. *Verify:* `cargo test -p cobolt-mcp`
  (new dispatch tests). *Serves:* R11, R13, R30.
- [x] **T3 (A1) — Shared content in `cobolt-project-tools`.** *(The `(rel, text)`
  redaction check moves to T10, where the bundle that needs it is written.)* Move rules, skills,
  reviewer, gap template and reference builders out of `agent_kit`; embed the two
  English docs for rcrun; rules point at resource URIs; redaction over
  `(rel, text)`. *Verify:* moved tests pass in their new crate; IDE builds.
  *Serves:* R7, R10, R13.
- [x] **T4 (A2b) — Instructions and resources on both servers.** *(The rules'
  `docs/powerrustcobol/…` wording is rewritten in T12, when the per-project kit
  that writes that folder is removed.)* `ProjectTools`
  answers `instructions` (rules + product version) and the reference resources;
  `Gated` and `IdeTools` forward them; rcrun reports the product version.
  *Verify:* AC5, AC7 tests over stdio and HTTP. *Serves:* R11, R13, R14.
- [x] **T5 (A2c) — `kb_search`.** *(Hashing scoring only; the optional cached
  semantic model is left out — the store's semantic records are scored by their
  text, as the assistant does without the model.)* Lift the chunk-store reader + hashing scorer
  into `cobolt-kb`; `chunked_knowledge` re-exports; installed store or in-memory
  fallback; optional cached semantic model; one excluded-documents constant;
  `kb_lookup` text fix. *Verify:* AC6. *Serves:* R12.
- [x] **T6 (A2d) — Project naming and mismatch.** `project` in every answer;
  optional `project` argument refused on mismatch with both names; project-free
  tools dispatch before project resolution. *Verify:* AC10. *Serves:* R19, R20.
- [x] **T7 (A2e) — `create_project` / `open_project` headless.** Extract the
  create core from `create_new_project_at` into `project_model`; host-trait
  methods; refuse non-empty folders. *Verify:* AC8. *Serves:* R15, R16, R18.
- [x] **T8 (A3) — IDE server: token, fixed path, live port, project requests.**
  `/mcp` + bearer token (401, constant time, never logged); rebind on port
  change; `HostRequest::{CreateProject, OpenProject}` refused while
  `has_unsaved_changes()`. *Verify:* AC9, AC16. *Serves:* R16, R17, R25, R35–R38.
- [ ] **T9 (A4) — rcrun: find the project, launch the IDE.** Upward search from
  `CLAUDE_PROJECT_DIR`/cwd; IDE `--open <path>`; launch sibling IDE and wait for
  its port. *Verify:* AC11, AC15. *Serves:* R20a, R21.
- [ ] **T10 (A5) — Plugin bundle + Configure.** Bundle writer into
  `llm::base_dir()/claude-code/marketplace/`; `CommandRunner` trait; `claude`
  discovery; marketplace add/update + install with config (per T1); errors shown,
  prior config kept. *Verify:* AC1–AC4 (recorder runner, bundle redaction,
  `claude plugin validate` on the bundle). *Serves:* R1–R10.
- [ ] **T11 (A6) — Claude Code Settings window.** Help menu item; leaderboard
  window pattern; status, port row moved from the Settings form, Configure/Update,
  Renew token; IDE-wide TOML. *Verify:* AC12, AC13, window-size test.
  *Serves:* R22–R27, R37.
- [ ] **T12 (A7) — Remove the per-project export.** File menu → Configure Claude
  Code; delete stamp/manifest/refresh-offer paths and their keys; new keys in six
  languages. *Verify:* AC14 (`i18n_tests`), IDE sweep. *Serves:* R1, R29.
- [ ] **T13 (A8) — Developer's Guide.** Rewrite the coding-agent section; delete
  its five translations. *Verify:* doc renders in the viewer test set.

## Phase B — seeing and running a form

- [ ] **T14 (B1) — CPU rasteriser.** `cobolt-forms::raster` over
  `ctx.tessellate`, textures applied, PNG out. *Verify:* pixel tests (solid rect,
  text present). *Serves:* R30, R31.
- [ ] **T15 (B2) — `render_form`.** Library-ise the golden test's render path;
  tool returns `Content::Image`; shell option via `ShellApp::new` + frame fn.
  *Verify:* AC17. *Serves:* R30.
- [ ] **T16 (B3) — `run_form`.** `prepare_form` refactor of `cmd_run_form`;
  `cobolt_form_host::headless`; hidden `rcrun run-form-headless`; subprocess
  with timeout, injected like `build::Builder`. *Verify:* AC18. *Serves:* R31.

## Phase C — building an application

- [ ] **T17 (C1) — Three application skills.** *Verify:* AC19. *Serves:* R32.
- [ ] **T18 (C2) — Patterns pack.** Five curated example forms as resources;
  load + `check` test. *Verify:* AC20. *Serves:* R33.
- [ ] **T19 (C3) — Architecture section and resource map in the instructions.**
  *Verify:* AC19. *Serves:* R34.

## Phase gate (end of each phase)

`cargo build --workspace`, then `cargo test --no-fail-fast` on every crate the
phase touched; report each `test result:` line. Operator end-to-end check after
Phase A: Help → Claude Code Settings → Configure; `claude` in a project, `/mcp`,
`kb_search`, `create_project`.
