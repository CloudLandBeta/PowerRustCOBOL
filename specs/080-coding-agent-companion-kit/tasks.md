<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Coding-agent companion kit (Claude Code first)

- **Status:** draft → awaiting approval (then `/implement`)
- **Plan:** ./plan.md (draft 2026-10-01; §9 F1 and F5 need an operator decision first)   **Date:** 2026-10-01
- **Branch:** `feat/coding-agent-kit`, landed on `features` (`git push origin HEAD:features`
  per `specs/steering/structure.md`).

Ordered, small, independently-verifiable tasks. Each names the code it **reads
first** (line numbers are from `ec18e93` and will have moved — re-read at the
moment of the task), the files it touches, the requirements and acceptance
criteria it serves, and how to verify it. Check off as completed.

## Standing rules for every task

- **One task = one commit**, and every commit bumps `z` in
  `crates/cobolt-ide/src/version.rs` and adds a dated top-of-file
  `CHANGELOG.md` entry (GOLDEN RULE, `specs/steering/tech.md`). Feature
  commits only; a fix found on the way goes to `fixes` from `main`, never here.
- **Every new test prints a quantified summary** (GOLDEN RULE #7): the cases it
  exercised by name and the counts it measured — never a bare pass.
- **Sweeps** use `--no-fail-fast` and every `test result:` line is read, never
  a grep for failures. `cobolt-ide` has no `[lib]`: its tests run with
  `--bin cobolt-ide`.
- **Phase gate** (end of each phase): `cargo build --workspace`, then
  `cargo test --no-fail-fast` on every crate the phase touched, plus
  `-p cobolt-mcp`, `-p cobolt-runtime --test test_mcp_tool_parity`,
  `-p cobolt-compiler --lib`, `-p cobolt-cli`,
  `-p cobolt-ide --bin cobolt-ide`. Known environmental reds are named, not
  chased (`libsqlite3-sys`, live-network tests, `test_external_crates_e2e`).
- **No System KB content change** anywhere in this list. If a task finds it
  needs one, stop and surface it: the KB rule then requires the table edit
  **and** `cargo run -p cobolt-ide --example build_chunked_kb` + the
  regenerated `assets/knowledge/chunked.data` in the same commit.

---

## Phase 0 — Groundwork in existing crates (no behaviour change)

- [ ] **T0.1 — `cobolt_mcp::dispatch`: one message in, one reply out** (R11, R11a; prerequisite of AC5, AC6a)
  - Read first: `crates/cobolt-mcp/src/server.rs:65-166`, `lib.rs:50-58`.
  - Files: `crates/cobolt-mcp/src/server.rs`, `crates/cobolt-mcp/src/lib.rs`.
  - Do: rename the private `handle_one` to `pub fn dispatch(raw: &[u8], handler:
    &mut H) -> Option<Response>`; `serve` calls it unchanged; re-export from
    `lib.rs`. No new dependency (065 R2/R3 contract, `lib.rs:7-48`).
  - Verify: `cargo test -p cobolt-mcp --no-fail-fast` — existing suite green;
    new test: a body with embedded newlines (pretty JSON) dispatches, and a
    notification returns `None`. `cargo tree -p cobolt-mcp` still shows serde
    + serde_json only.

- [ ] **T0.2 — `cobolt_compiler::system_documentation()`** (R7; prerequisite of AC4)
  - Read first: `crates/cobolt-compiler/src/lib.rs:3968-4977`;
    `crates/cobolt-ide/src/grace_host.rs:4498` (the freshness test).
  - Files: `crates/cobolt-compiler/src/lib.rs`.
  - Do: `pub fn system_documentation() -> Vec<(&'static str, String)>`
    returning the eight `(file name, text)` pairs; `publish_system_documentation`
    becomes a loop that writes them. Text **byte-identical**.
  - Verify: `cargo test -p cobolt-compiler --lib -- published_documentation every_control_the_toolbox`;
    new test: the pairs equal what `publish_system_documentation` writes, file
    by file (prints names + byte counts);
    `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    green with **no** change to `assets/knowledge/chunked.data`.

- [ ] **T0.3 — Manifest view + one reseal rule in the compiler** (R11; prerequisite of T1.7)
  - Read first: `cobolt-compiler/src/lib.rs:728-757, 868-921`;
    `main_form_guard.rs:88-185`; `cobolt-ide/src/project_model.rs:1449-1489`
    and its `main_form_seal_tests` (`1492-…`); `project_upgrade.rs:44-52`.
  - Files: `crates/cobolt-compiler/src/lib.rs`,
    `crates/cobolt-compiler/src/main_form_guard.rs`,
    `crates/cobolt-ide/src/project_model.rs`.
  - Do: `pub fn project_manifest_view(manifest) -> Result<ManifestView, String>`
    (project name, `structure`, file lists incl. `indexed`, `[[crates]]` lib
    names) in the `project_file_memory_limit` style.
    `pub fn designation_record(structure, dir, name, forms) -> (String, String)`
    returning `(main_form, seal)` — empty pair below
    `STRUCTURE_MAIN_FORM_SEAL` or on a double designation — and `save_project`
    calls it instead of its inline block.
  - Verify: `cargo test -p cobolt-compiler --lib -- main_form`;
    `cargo test -p cobolt-ide --bin cobolt-ide main_form_seal_tests`;
    `cargo test -p cobolt-cli --test main_form_gate` — all green, unchanged
    assertions.

- [ ] **T0.4 — `docs_embed::embedded_doc`** (R7; prerequisite of AC4)
  - Read first: `crates/cobolt-ide/src/docs_embed.rs:21-135`.
  - Files: `crates/cobolt-ide/src/docs_embed.rs`.
  - Do: `pub fn embedded_doc(name: &str) -> Option<&'static str>` over `DOCS`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` — new test
    resolves `developers-guide-en.md` and `cobol85-supported-syntax-en.md` and
    prints their byte sizes; the existing guards behave as before.

- [ ] **T0.5 — Make the AI-export redaction reusable** (R4; prerequisite of AC3)
  - Read first: `crates/cobolt-ide/src/ai_bundle.rs:160-286, 530-546`.
  - Files: `crates/cobolt-ide/src/ai_bundle.rs`.
  - Do: `Personal::scrub`, `Personal::find`, `find_key` → `pub(crate)`. No
    logic change.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide ai_bundle` — green,
    same counts as before.

**Gate 0:** the phase gate; `git diff --stat` shows only the five files above
plus version/CHANGELOG.

---

## Phase 1 — `cobolt-project-tools`: the one tool set (headless, fully tested)

- [ ] **T1.1 — Create the crate with path confinement** (R12, R13; AC5)
  - Read first: `Cargo.toml:8-31`; `crates/cobolt-mcp/Cargo.toml`;
    `cobolt-compiler/src/lib.rs:1043-1076` (`find_project_manifest`).
  - Files: `Cargo.toml` (member after `cobolt-mcp`),
    `crates/cobolt-project-tools/{Cargo.toml,src/lib.rs,src/root.rs}`.
  - Do: dependencies per plan §2. `ProjectRoot::open(manifest_or_dir)` (a
    folder must hold the manifest itself — an ancestor's is refused) and
    `resolve(rel)`: refuse absolute, `..`, and real paths leaving the
    canonical root (symlink).
  - Verify: `cargo build -p cobolt-project-tools`; `cargo test -p
    cobolt-project-tools root` — `../x`, `/etc/passwd`, `C:\x`, a symlink out,
    and an ancestor manifest are refused; in-project paths resolve. Prints
    cases × verdicts. `cargo tree -p cobolt-project-tools -i cobolt-ide`
    finds nothing (no cycle).

- [ ] **T1.2 — `ProjectHost` + `HeadlessHost`** (R11, R14; AC5)
  - Read first: plan §1.5; `project_model.rs:852-878`; T0.3's API.
  - Files: `crates/cobolt-project-tools/src/host.rs`.
  - Do: the trait of plan §1.5; `HeadlessHost::record` edits `[files] <list>`
    as a `toml::Value` (dedupe, `/` separators) and writes the
    `designation_record` result into `[forms] main-form` / `main-form-seal`;
    an unreadable manifest makes `project()` return `NoProject`.
  - Verify: `cargo test -p cobolt-project-tools host` — record a new form;
    `cobolt_compiler::main_form_guard::authorize_form_start` on the main form
    returns `Allowed` (the seal verifies); unknown manifest keys survive the
    round trip; unreadable manifest → `NoProject`.

- [ ] **T1.3 — Move the generated-path rule** (R11; AC6)
  - Read first: `cobolt-ide/src/app.rs:7312-7388, 20045-20059`.
  - Files: `crates/cobolt-project-tools/src/gen_paths.rs`,
    `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/Cargo.toml`.
  - Do: `generated_cbl_path(generated_list, dir, cfrm)` and
    `generated_indexed_cbl_path(..)` as pure functions; the IDE methods
    delegate (no `CoboltApp` logic left in them).
  - Verify: `cargo test -p cobolt-project-tools gen_paths` (default, relocated
    entry, loose form); `cargo test -p cobolt-ide --bin cobolt-ide` — the
    existing generated-path tests green.

- [ ] **T1.4 — Move the form/source validation and the binding guardian** (R11; AC5, AC6a)
  - Read first: `cobolt-ide/src/app.rs:4058-4115, 4181-4245, 2349-2356`;
    `data_binding_guardian.rs` (whole; confirm still no `crate::` imports);
    `external_crates_service.rs:112-127`.
  - Files: `crates/cobolt-project-tools/src/{validate_source.rs,binding_guardian.rs}`
    (`git mv` of `data_binding_guardian.rs`),
    `crates/cobolt-ide/src/data_binding_guardian.rs` (re-export shim),
    `crates/cobolt-ide/src/app.rs`.
  - Do: `validate_form_source(form, program_path, external_crates) ->
    (Vec<Diag>, String, SourceMap)` and `validate_source(path, text,
    external_crates)` with the IDE's fixed/free heuristic; the IDE's
    `validate_form_source_full` and `do_check` call them with
    `analyze_project`'s crate list. Diagnostic type stays a plain struct the
    IDE maps into its `DiagMsg`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — the
    moved guardian tests (now under `-p cobolt-project-tools`) and every IDE
    test green; counts reported before/after the move are equal.

- [ ] **T1.5 — Tool `list_files`** (R11; AC5)
  - Files: `crates/cobolt-project-tools/src/tools/list.rs`.
  - Do: plan §1.6 row.
  - Verify: `cargo test -p cobolt-project-tools list` on a fixture project
    (forms, an indexed file, a source, two reports) — every list and the
    reports appear with correct exists flags.

- [ ] **T1.6 — Tool `check`** (R11; AC5)
  - Read first: `cobolt-codegen/src/lib.rs:129-140`; `cobolt-ide/src/app.rs:550-586`;
    `main_form_guard.rs:134-185`.
  - Files: `crates/cobolt-project-tools/src/tools/check.rs`,
    `crates/cobolt-project-tools/tests/fixtures/check_project/**`.
  - Do: whole project or one `path`; generated programs validated in memory
    (nothing written); form diagnostics mapped through `SourceMap::resolve`
    to `{file: <.cfrm>, form, site, line}`; codegen-authored lines keep the
    generated file; designation `Err` and binding blockers are errors.
  - Verify: `cargo test -p cobolt-project-tools check` — a planted undeclared
    item in an `onClick` handler reports the `.cfrm`, the site and the line
    inside the handler; a second main form is an error; a clean project
    reports 0 errors. Prints diagnostics found per case.

- [ ] **T1.7 — Tools `regenerate` and `add_to_project`** (R11; AC6; plan D4, D5)
  - Read first: `cobolt-ide/src/app.rs:7232-7256, 7500-7538, 11562-11610`;
    `project_model.rs:852-878, 1152`.
  - Files: `crates/cobolt-project-tools/src/tools/{regenerate.rs,register.rs}`.
  - Do: `regenerate {path?}` (form → `.cbl`; `.cidx` → `-indexed.cbl` + `.SEL`/`.FD`
    in `COPYBOOKS/`); refuses a target `host.unsaved()` reports; `record`s
    each written file as generated; `written()` names them.
    `add_to_project {path, list?}` routes by extension, `record`s.
  - Verify: `cargo test -p cobolt-project-tools regenerate register` — the
    written `.cbl` is byte-equal to `cobolt_codegen::generate_with_map(load_form(..)).0`
    at the path `gen_paths` gives; an unsaved target is refused and nothing
    is written; a registered form appears in the manifest and the seal still
    verifies. **AC6 (regenerate half).**

- [ ] **T1.8 — Tool `build` with bounded wait** (R11; AC6)
  - Read first: `cobolt-compiler/src/lib.rs:1153-1256`;
    `cobolt-ide/src/app.rs:4767-4900`.
  - Files: `crates/cobolt-project-tools/src/tools/build.rs`,
    `crates/cobolt-project-tools/tests/build.rs`.
  - Do: regenerate all → `check` errors refuse → `build_project` on a worker
    (`verbose: false`, `workspace_root` from the host); wait ≤ 40 s; a repeat
    call joins the running build (plan §1.6).
  - Verify: `cargo test -p cobolt-project-tools build` (fast: refusal on a
    check error; join-not-restart with a stub builder). **AC6 (build half):**
    `cargo test -p cobolt-project-tools --test build -- --ignored` builds a
    one-form fixture, reports elapsed time, binary path and size.

- [ ] **T1.9 — Tools `validate` and `kb_lookup`** (R11; AC5)
  - Read first: `cobolt-indexed/src/{xml.rs:35-50, structure.rs:159, schema_support.rs:14}`;
    `cobolt-compiler/src/lib.rs:5089, 6252, 6767-6990, 7364`;
    `cobolt-runtime/src/builtins.rs:35-90`.
  - Files: `crates/cobolt-project-tools/src/tools/{validate.rs,kb.rs}`.
  - Do: plan §1.6 rows; `kb_lookup` slices the `## Control:` section (or the
    method/property/event entry) out of `system_documentation()`'s text and
    reads `builtin(name)`; output capped.
  - Verify: `cargo test -p cobolt-project-tools validate kb` — broken XML,
    a REDEFINES-forward `.cidx` and a clean pair; `kb_lookup` finds `Button`,
    `Caption`, `onClick`, `AddRow`, `HTTP-GET`, and answers "not in the
    reference" for an invented name (the R19 basis). Prints hits/misses.

- [ ] **T1.10 — `ProjectTools: McpHandler`, no-project answers** (R11, R13, R14; AC5)
  - Read first: `cobolt-runtime/src/mcp_tool.rs:484-499` (the model).
  - Files: `crates/cobolt-project-tools/src/lib.rs`,
    `crates/cobolt-project-tools/tests/tools.rs`.
  - Do: seven tools with JSON schemas and English descriptions (R22); the
    impl delegates and holds no logic; `server_info` version from the host;
    every tool answers "no project open" when `host.project()` is `NoProject`.
  - Verify: `cargo test -p cobolt-project-tools --test tools` — over
    `cobolt_mcp::serve` with a scripted client: `initialize`, `tools/list` = 7,
    `check` on the error fixture returns file + line, `../` and an absolute
    path refused, no-project answers on every tool. **AC5 (tools half).**

- [ ] **T1.11 — Minimal Streamable-HTTP transport** (R11a, R12, R14; AC5)
  - Read first: plan §1.4; `cobolt-mcp/src/transport.rs:25`.
  - Files: `crates/cobolt-project-tools/src/http.rs`,
    `crates/cobolt-project-tools/tests/http.rs`.
  - Do: `serve_http(listener, make_handler, kit_id_of_open_project)` with the
    exact table of plan §1.4; thread per connection; write tools serialised
    on one mutex.
  - Verify: `cargo test -p cobolt-project-tools --test http` on
    `127.0.0.1:0`: initialize → 200 JSON; notification → 202; GET/DELETE → 405;
    foreign `Origin` → 403; `Host: evil.example` → 403; `text/plain` → 415;
    chunked → 411; oversize → 413; wrong kit id → the "different project"
    answer; the listener's local address is loopback. **AC5 (transport half).**

**Gate 1:** the phase gate + `cargo test -p cobolt-project-tools --no-fail-fast`
+ the `--ignored` build test once.

---

## Phase 2 — `rcrun mcp` (headless, stdio)

- [ ] **T2.1 — `rcrun mcp [--project <manifest|folder>]`** (R11, R11a, R12, R14; AC6a)
  - Read first: `crates/cobolt-cli/src/main.rs:56-100, 343-391`;
    `crates/cobolt-cli/Cargo.toml`; `crates/cobolt-cli/tests/main_form_gate.rs`
    (how the CLI is tested).
  - Files: `crates/cobolt-cli/src/main.rs`, `crates/cobolt-cli/Cargo.toml`,
    `crates/cobolt-cli/tests/mcp_stdio.rs`.
  - Do: dispatch arm; tracing to **stderr** when the subcommand is `mcp`;
    `HeadlessHost` from `--project` or the working directory;
    `cobolt_mcp::serve(stdin.lock(), stdout.lock(), &mut tools)`; help text.
  - Verify: `cargo test -p cobolt-cli --test mcp_stdio` — `initialize`,
    `tools/list` equal (names + schemas) to the in-process list, `check` equal
    to the in-process result on the error fixture, every stdout line parses
    as JSON-RPC, an unreadable manifest makes every tool answer "no project";
    no `TcpListener` in `crates/cobolt-cli/src`; on Unix with `lsof`, the
    process holds no inet socket (else skipped with the reason printed).
    **AC6a.**

**Gate 2:** the phase gate.

---

## Phase 3 — The IDE serves the tools over HTTP

- [ ] **T3.1 — MCP-port setting** (R11a, R21; AC11)
  - Read first: `cobolt-ide/src/llm.rs:62-65, 296-304, 330-340, 880-895`;
    `panels/settings_form.rs:85, 184, 310-321, 1475-1500`;
    `i18n.rs:462` (`ai_inspection_port`, all six tables).
  - Files: `crates/cobolt-ide/src/llm.rs`,
    `crates/cobolt-ide/src/panels/settings_form.rs`,
    `crates/cobolt-ide/src/i18n.rs`.
  - Do: `mcp_port` (default 5720) persisted like `inspection_port`; Settings
    row beside it (range 1024–65535, must differ from the inspection port);
    `Tr` `ai_mcp_port`, `ai_mcp_port_hint` ("takes effect on restart;
    re-export the coding-agent kit") ×6.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide llm settings_form i18n`
    — an old config gets 5720; a saved value round-trips; equal-to-inspection
    is rejected.

- [ ] **T3.2 — `IdeHost` and the listener** (R11, R12, R14; AC5)
  - Read first: `app.rs:1980-1994, 2182-2193, 14565-14580` (startup, Output
    line, per-frame publish); `app.rs:1058, 2165` (an `mpsc` receiver drained
    in `update`); `designer.rs:2501`; `editor.rs:2088, 2118`;
    `app.rs:10898-10903`.
  - Files: `crates/cobolt-ide/src/agent_kit/ide_host.rs`,
    `crates/cobolt-ide/src/agent_kit/mod.rs`, `crates/cobolt-ide/src/main.rs`,
    `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`.
  - Do: the per-frame snapshot (project dir, manifest, kit id, unsaved paths,
    building flag, crates); listener started at app creation on
    `127.0.0.1:<mcp_port>`; `record`/`written` sent to the UI thread and
    applied with `add_file_to`/`add_generated` + `do_save_project`,
    `editor.reload_file`, form re-validation; `request_repaint()` from the
    worker; one Output line per tool call (`mcp_activity`); `Tr`
    `ai_mcp_listening`, `ai_mcp_failed`, `mcp_activity` ×6.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::ide_host` —
    with a stub snapshot: no project → "no project open"; other kit id → the
    "different project" answer; a `record` round-trip through the channel
    reaches the project model and the manifest is saved once.

- [ ] **T3.3 — Conflict rules with the open IDE** (R11, R14; plan §5 risk 1)
  - Read first: `app.rs:1495-1533` (`InspectState::reload_if_stale`).
  - Files: `crates/cobolt-ide/src/agent_kit/ide_host.rs`,
    `crates/cobolt-ide/src/app.rs`.
  - Do: writing tools refuse a target with unsaved IDE edits (any form for
    `build`), naming the file; `build` refuses while the IDE builds; on every
    MCP call, clean open designers and the inspector whose `.cfrm` changed on
    disk reload.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::conflicts` —
    a pure decision function over (snapshot, tool, target) covers each case
    and prints the table of decisions.

**Gate 3:** the phase gate; then, manually, start the IDE and `curl -s -X POST
-H 'Content-Type: application/json' http://127.0.0.1:5720/mcp/x -d
'{"jsonrpc":"2.0","id":1,"method":"tools/list"}'` lists seven tools (record the
output in the commit message).

---

## Phase 4 — The kit generator

- [ ] **T4.1 — Target-neutral content model** (R8, R9, R10, R16, R17, R18, R19, R20, R22; AC8)
  - Read first: plan §1.7; spec R8–R10, R16–R19.
  - Files: `crates/cobolt-ide/src/agent_kit/content.rs`.
  - Do: `KitContent` (Serialize) and `content::build(ctx)`; the brief's rules
    (R8 list verbatim as instructions, R16, R19, "never edit the manifest —
    use `add_to_project`", "prefer `powerrustcobol-ide`; use `powerrustcobol`
    only when the IDE is closed", "ask the developer to save or close a form a
    tool says is unsaved"); seven skills (R9) naming the tools each uses; the
    reviewer (R10); the gap-report template with the seven R18 fields and the
    path rule `docs/compiler-requests/<YYYY-MM-DD>-<topic>.md` (R17). All text
    English; COBOL examples inline `COBOL::"…"`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::content` —
    **AC8**: every R18 field in the template; every R8 rule in the brief;
    seven skills with the R9 subjects. Prints the field/rule/skill tally.

- [ ] **T4.2 — Reference pack** (R7, R22; AC4)
  - Read first: T0.2/T0.4 APIs; `cobolt-forms/src/xml.rs:7-51` (format prose
    source); `cobolt-forms/src/model.rs:1395, 3063`;
    `cobolt-compiler/src/lib.rs:4990-5076, 6252`.
  - Files: `crates/cobolt-ide/src/agent_kit/reference.rs`.
  - Do: plan §1.7 table — KB documents (minus `agents_registry.md`,
    `ide_functionalities.md`), `builtins.md` from `BUILTINS`, the whole Guide,
    the syntax doc, generated `cfrm-format.md`/`cidx-format.md` with a live
    example, `README.md` index.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::reference` —
    **AC4**: per control type, every seeded/runtime property, every supported
    event and every documented method is in the pack; every `BUILTINS` name is
    in `builtins.md`; every serialised `.cfrm`/`.cidx` element and attribute
    is named in the format docs. Prints the five counts and any miss.

- [ ] **T4.3 — Claude Code writer + a test-only second writer** (R2, R15, R20; AC7, AC9)
  - Read first: plan §1.7, §3.3, §4 D10, §8 A1/A3/A4;
    `project_model.rs:1675-1688` (`find_cobolt_binary`).
  - Files: `crates/cobolt-ide/src/agent_kit/claude_code.rs`,
    `crates/cobolt-ide/src/project_model.rs` (`find_cobolt_binary` →
    `pub(crate)`).
  - Do: `ClaudeCodeWriter: KitWriter` → `CLAUDE.md` section,
    `.claude/settings.json` (plan D10 rules), seven `SKILL.md`, the agent file,
    `.mcp.json` (http URL with `kit_id` + port; stdio with
    `${HOME}`-prefixed or absolute rcrun and `["mcp","--project","${CLAUDE_PROJECT_DIR}"]`),
    pack under `docs/powerrustcobol/`. `#[cfg(test)] FlatWriter`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::claude_code` —
    **AC7** (plan §6 row: parses; required rules present; nothing granting
    outside); **AC9**: `KitContent` JSON identical before/after both writers
    render, layouts differ; frontmatter is the first bytes of every
    `SKILL.md`/agent file; no home path in `.mcp.json` when the rcrun path is
    under a fake home.

- [ ] **T4.4 — Stamps, kit manifest and ownership** (R3, R5; AC1, AC2)
  - Read first: plan §3.2–§3.3, §4 D9.
  - Files: `crates/cobolt-ide/src/agent_kit/stamp.rs`.
  - Do: version stamps per file kind; `CLAUDE.md` section merge (append /
    replace in place / keep when edited); JSON merge of kit-owned keys;
    per-file decision `Write | Replace | KeepEdited | Merge`; `kit_id` kept
    across exports; manifest written last.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::stamp` —
    **AC2**: developer text around the section byte-equal after re-export; an
    edited skill kept and reported; a deleted kit file re-written; a
    developer's own `.mcp.json` server survives a merge. Prints bytes compared
    and the decision per file.

- [ ] **T4.5 — Redaction and refusal** (R4; AC3)
  - Read first: `ai_bundle.rs:167-286, 535-544`; plan §4 D8, §9 F1.
  - Files: `crates/cobolt-ide/src/agent_kit/redact.rs`,
    `crates/cobolt-ide/src/i18n.rs` (`agent_kit_refused`,
    `agent_kit_refused_key`, `agent_kit_refused_personal` ×6).
  - Do: scrub interpolated values with `Personal::scrub`; scan every final file
    with `find_key`, every generated file with `Personal::find`; any hit →
    refuse before the first write.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::redact` —
    **AC3**: needles planted in the project name and the rcrun path are
    replaced; a stored key planted anywhere refuses and the temp project's
    directory hash is unchanged. Prints needles planted, files scanned,
    replacements, refusals.

- [ ] **T4.6 — `export()` end to end** (R1, R2, R5; AC1)
  - Files: `crates/cobolt-ide/src/agent_kit/mod.rs`.
  - Do: `export(project_dir, target, ctx) -> Result<ExportReport, Refusal>`
    composing T4.1–T4.5; the report lists every file with its decision.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::tests::export_into_powerchat_copy`
    — **AC1** on a temp copy of `examples/PowerChat` (manifest, `forms/`,
    `src/`): every R2 path written, each stamped, every JSON file in the
    manifest; a second export replaces nothing it should keep. Prints files by
    kind and decision.

**Gate 4:** the phase gate.

---

## Phase 5 — IDE surfaces

- [ ] **T5.1 — File ▸ Export coding-agent kit ▸ Claude Code** (R1, R21; AC1, AC11)
  - Read first: `app.rs:15079-15118`; `app.rs:13927-13961` (how the AI export
    reports to Output).
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`.
  - Do: submenu after Package Project, enabled with a project; runs
    `agent_kit::export` with the open project, the IDE version, `mcp_port`,
    `Personal::from_environment()`, the stored keys; pushes one Output line per
    file (`agent_kit_wrote` / `agent_kit_kept_edited` / `agent_kit_merged`)
    and `agent_kit_done`, or the refusal. `Tr` `menu_export_agent_kit`,
    `menu_export_agent_kit_hint`, `agent_kit_target_claude_code`,
    `agent_kit_wrote`, `agent_kit_kept_edited`, `agent_kit_merged`,
    `agent_kit_done` ×6.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide --bin
    cobolt-ide i18n agent_kit`; manual: export into a scratch copy of
    PowerChat, Output lists every file (screenshot in the commit message).

- [ ] **T5.2 — Refresh offer on project open** (R6, R21; AC11)
  - Read first: `app.rs:4504-4605, 12083-12142`.
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/agent_kit/mod.rs`,
    `crates/cobolt-ide/src/i18n.rs`.
  - Do: after `detect_project_upgrades`, read the kit manifest; a version ≠
    `version::VERSION` raises a fixed-size modal (`.resizable(false)`, the
    upgrade modal's shape) shown after it: Refresh (runs T5.1's export) / Not
    now (offered again next open). `Tr` `agent_kit_refresh_title`,
    `agent_kit_refresh_detail`, `agent_kit_refresh_apply`,
    `agent_kit_refresh_later` ×6.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide agent_kit::refresh` —
    pure `needs_refresh(manifest, version)`: equal → no; older/newer → yes;
    unreadable → no (no kit). Manual: open a project whose manifest says an
    older version — the offer appears once.

- [ ] **T5.3 — Compiler requests node** (R19a, R21; AC6b, AC11)
  - Read first: `panels/project.rs:52-90, 600-620, 955-1000`;
    `app.rs:958-966` (mtime cache pattern).
  - Files: `crates/cobolt-ide/src/panels/project.rs`,
    `crates/cobolt-ide/src/i18n.rs`.
  - Do: after the `Category::TOP` loop, a read-only node listing
    `<root>/docs/compiler-requests/*.md`, newest first, cached on the folder's
    mtime, absent when empty; a row click emits `ProjectPanelEvent::Open`.
    `Tr` `cat_compiler_requests` ×6.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide compiler_requests` —
    **AC6b**: two reports → both, newest first; none → no node; a non-`.md`
    file is ignored. Manual: the node appears without reopening the project.

**Gate 5:** the phase gate; `cargo run -p cobolt-ide` launches and the three
surfaces work (operator glance).

---

## Phase 6 — Documentation, KB, i18n

- [ ] **T6.1 — Guide, dependency table, capability row, registry** (R1–R19a user-facing; spec §6 Docs/Security)
  - Read first: `docs/developers-guide-en.md` ToC (30-66), §16 (9468), §17
    (11055); `docs/DEPENDENCIES-en.md:40-58`; `docs/cobol-support-matrix-en.md`;
    `specs/steering/docs.md` registry; `specs/steering/doc-style.md`.
  - Files: `docs/developers-guide-en.md`, `docs/DEPENDENCIES-en.md`,
    `docs/cobol-support-matrix-en.md`, `specs/steering/docs.md`; **deleted**:
    `docs/DEPENDENCIES-{es,pt,fr,jp,cn}.md`,
    `docs/cobol-support-matrix-{es,pt,fr,jp,cn}.md`.
  - Do: Guide subsection "Working with a coding agent" (export, what each file
    is, the two servers and when each is used, the tools, gap reports and the
    Compiler requests node, the MCP-port setting, **no authentication: anything
    on this machine that can reach the port can call the tools**, the seal
    reason for `add_to_project`); `rcrun mcp` in §17. Dependencies: the new
    crate row (and the already-missing `cobolt-mcp`, `cobolt-docs`,
    `cobolt-kb` rows). Matrix: one PowerRustCOBOL-extension row. Registry rows
    for `crates/cobolt-project-tools/**`, `crates/cobolt-ide/src/agent_kit/**`.
    No Guide translation exists to delete (`ls docs/developers-guide-*` →
    `-en` only).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` — the two
    translation guards go **red** for `DEPENDENCIES` and
    `cobol-support-matrix` (intended until the next minor; do not re-add
    `#[ignore]`); every other docs test green. No COBOL sample uses `CALL
    "COBOL-…"`.

- [ ] **T6.2 — i18n completeness** (R21; AC11)
  - Files: `crates/cobolt-ide/src/i18n.rs` (test module).
  - Do: `agent_kit_strings_in_every_language` over the 20 keys of plan §3.4.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` — **AC11**: each
    key non-empty in six tables, `{}` counts equal to English; prints keys ×
    languages checked.

- [ ] **T6.3 — KB gate confirmation** (steering: System KB)
  - Files: none unless a gap appears.
  - Do: confirm no control/property/method/event or runtime behaviour changed
    in this branch (`git diff main -- crates/cobolt-forms crates/cobolt-runtime
    crates/cobolt-codegen` empty; the compiler diff is T0.2/T0.3 only).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    green and `assets/knowledge/chunked.data` unchanged.

---

## Phase 7 — Finalize

- [ ] **T7.1 — Full sweep and coverage** (all)
  - Do: `cargo test --workspace --no-fail-fast` (`cobolt-forms` with
    `--features render`, `cobolt-ide` with `--bin`), every `test result:` line
    read; the `--ignored` build test once; confirm each AC below has a green,
    measured verification and say which do not.
  - Verify: the coverage table below filled with the test names and their
    printed numbers.

- [ ] **T7.2 — AC10, end to end — MANUAL, operator-run** (AC10; R1–R19a)
  - Not automatable: it needs Claude Code, a model, and judgement.
  - Steps for the operator:
    1. New desktop project in the IDE; File ▸ Export coding-agent kit ▸ Claude
       Code; Output lists every file.
    2. `claude` in the project folder; approve the two project MCP servers;
       `/mcp` shows both connected (IDE open) — this also checks §8 A1, A2, A5.
    3. Ask for a record-entry form over a new indexed file. Expect: `.cidx`
       written, `add_to_project` and `regenerate` called, `check` clean, form
       runs from the IDE.
    4. Ask for one capability the product lacks. Expect a
       `docs/compiler-requests/<date>-<topic>.md` with every R18 field, the
       supported part done, and the Compiler requests node showing it.
    5. Close the IDE; ask for a small change; expect the `powerrustcobol`
       (stdio) server used, `check` clean.
    6. Confirm nothing outside the project changed (`git status` of the
       PowerRustCOBOL checkout clean; Claude Code transcript shows no edit
       outside the project) and that §8 A3/A4/A6 held.
  - Record: pass/fail per step, and every assumption of plan §8 found false.

## Acceptance-criteria coverage

| AC | Task(s) | AC | Task(s) |
|---|---|---|---|
| AC1 | T4.4, T4.6, T5.1 | AC6a | T2.1 |
| AC2 | T4.4 | AC6b | T5.3 |
| AC3 | T0.5, T4.5 | AC7 | T4.3 |
| AC4 | T0.2, T0.4, T4.2 | AC8 | T4.1 |
| AC5 | T1.1, T1.2, T1.6, T1.10, T1.11, T3.2 | AC9 | T4.3 |
| AC6 | T1.3, T1.7, T1.8 | AC10 | T7.2 (manual) |
| | | AC11 | T3.1, T3.2, T4.5, T5.1–T5.3, T6.2 |

Requirement traceability: R1 T5.1 · R2 T4.3, T4.6 · R3 T4.4 · R4 T4.5 ·
R5 T4.4 · R6 T5.2 · R7 T0.2, T0.4, T4.2 · R8–R10 T4.1 · R11 T1.1–T1.10, T2.1,
T3.2 · R11a T1.11, T2.1, T3.1 · R12 T1.1, T1.11, T2.1 · R13 T1.1, T1.10 ·
R14 T1.2, T1.10, T1.11, T3.2 · R15 T4.3 · R16–R19 T4.1 · R19a T5.3 ·
R20 T4.1, T4.3 · R21 T3.1, T3.2, T4.5, T5.1–T5.3, T6.2 · R22 T4.1, T4.2.

## Done criteria

AC1–AC9 and AC11 green with measured results; AC10 run by the operator and
recorded; the workspace green apart from named environmental failures and the
two intended `docs_embed` reds; the Guide updated and the ten invalidated
translations deleted; every commit a feature commit with its own `z` bump and
CHANGELOG entry, on `features`, with no commit or push unless the operator
asks.
