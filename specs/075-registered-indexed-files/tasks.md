# Tasks — Registered indexed files (`AgentObject::RegisterFile`)

- **Status:** ready for `/implement`
- **Plan:** ./plan.md   **Date:** 2026-09-24

Ordered so the workspace stays green after each task. Each commit bumps `z` and adds a
CHANGELOG entry; `features` branch. Tests print quantified results (GOLDEN RULE #7). No
test ever changes a registered file: every test is wrapped by `assert_untouched` (AC7).

## Phase 1 — the memory limit becomes a project setting

- [ ] **T1 — Runtime global** (065 R34)
  - Files: `crates/cobolt-runtime/src/mcp_tool.rs`.
  - Do: `publish_file_memory_limit(bytes)` / `file_memory_limit()` (process global, first
    call wins, default `DEFAULT_MEMORY_LIMIT_BYTES`); `IndexedToolSet::default` reads it.
  - Verify: unit test — unpublished → 64 MiB; published value reaches a new tool set.

- [ ] **T2 — Project manifest, three hosts**
  - Files: `crates/cobolt-compiler/src/lib.rs` (`AgentsConfig`,
    `project_file_memory_limit`, generated `const PROJECT_FILE_MEMORY_LIMIT_MB` + publish
    call in `run_form_app`), `crates/cobolt-cli/src/form_gui.rs`.
  - Do: `[agents] file_memory_limit_mb` (0/absent → 64); rcrun publishes it beside
    `publish_connections`; child forms inherit the process value.
  - Verify: compiler tests — manifest parse, generated source carries the const and the call.

- [ ] **T3 — IDE Project Settings**
  - Files: `crates/cobolt-ide/src/project_model.rs` (`AgentsSettings`),
    `panels/settings_form.rs` (Runtime section, 1–65536 MB), `i18n.rs`
    (`lbl_runtime_file_memory_limit`, `hint_runtime_file_memory_limit` ×6).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` (i18n completeness, settings
    round-trip); an old `cobolt.toml` without `[agents]` loads and saves unchanged.

## Phase 2 — reading a file by path, safely

- [ ] **T4 — Engine helpers** (R4, R15, R16)
  - Files: `indexed.rs` (`IndexedFile::inspect_bytes`), `indexed_disk.rs`
    (`input_needs_copy(path)`, `pub(crate) fn schema_equivalent` extracted from
    `schema_matches`).
  - Verify: unit tests — each reads without write access; `schema_matches` behaviour
    unchanged (existing indexed suites green).

- [ ] **T5 — `registered_file.rs`: location, `SmbUrl`, masking** (R7–R9, R11–R13)
  - Do: `Location::{Fs, Smb}`, relative paths via `assets::resolve`; `SmbUrl` parse
    (`domain;user:password@server:port/share/path`, percent-decoding), `Display`/`Debug`
    masked.
  - Verify: table tests of path forms per OS; no formatting of an `SmbUrl` contains the
    password.

- [ ] **T6 — `.cidx` validation and `from_registered_definition`** (R3–R5)
  - Files: `registered_file.rs`, `mcp_tool.rs` (`FileAccess.alternates`).
  - Do: `validate_definition`, leaf offsets/lengths, record length, declared vs stored
    schema, multi-part key → `KEY-MISMATCH`; R5 purpose / fields / descriptions.
  - Verify: one fixture per refusal code (AC2).

- [ ] **T7 — Fit decision and free memory** (R17–R20)
  - Do: pure `decide(container, location, size, limit, free)`; `trait FreeMemory` (`sysinfo`
    available memory, added to `cobolt-runtime`), injectable probe.
  - Verify: table tests — every branch, both numbers in each refusal (AC10–AC12 at unit
    level).

- [ ] **T8 — `FileSource` in the tool set** (R2, R14, R15, R19, R21)
  - Files: `mcp_tool.rs`.
  - Do: `Assigned` (unchanged) / `Loaded(Arc<Vec<Bytes>>)` / `InPlace(PathBuf)` (re-check
    `.jrn` before each search; `DiskIndexedFile` `INPUT` with all keys);
    `register`/`unregister`; amend the R30 note.
  - Verify: `test_mcp_tool_parity` and existing `AllowFile` tests unchanged; new unit tests
    for `Loaded` and `InPlace` scans.

## Phase 3 — COBOL surface

- [ ] **T9 — `RegisterFile` / `UnregisterFile`** (R1, R2, R6, R6a, R10, R14–R16)
  - Files: `interpreter.rs` (`exec_method`, `is_known_method`),
    `interpreter/agent_loop.rs`, `cobolt-forms/src/model.rs` (five run-time properties).
  - Do: order of checks per plan §2; properties written every call; logs carry the masked
    location only.
  - Verify: `crates/cobolt-runtime/tests/test_registered_files.rs` with the mock model
    server — AC1, AC2, AC3, AC5 (local), AC7, AC8 (read-only file and folder), AC9, AC10,
    AC11, AC12 (`PRCIDX1`); `is_known_method` sync test green.

## Phase 4 — `smb://`

- [ ] **T10 — `smb2` fetch** (R9, R11–R13, R20)
  - Files: `crates/cobolt-runtime/{Cargo.toml,src/smb_source.rs}`,
    `crates/cobolt-form-host/Cargo.toml`, `crates/cobolt-cli/Cargo.toml`.
  - Do: feature `smb` (`smb2 = "=0.26.0"`, tokio); `trait SmbFetch` (`stat`, `read_all`
    bounded); `RealSmb` on its own thread with a current-thread runtime, 30 s deadline; guest
    = `Guest` + empty password; without the feature → `SMB-UNAVAILABLE`; temp copy for a
    `PRCIDXD1` deleted by a drop guard.
  - Verify: fake `SmbFetch` — same tool output as the local path (AC4), unreachable and
    login-refused codes (AC5), password absent from logs, properties and model requests
    (AC6), oversize refused with both numbers (AC12); `cargo tree -p cobolt-runtime
    --features smb` has no `ring`, `cc` or `-sys` crate.

- [ ] **T11 — Build features and parity** (R22, R23)
  - Files: `crates/cobolt-compiler/src/runtime_features.rs`.
  - Do: `RuntimeFeatures.smb` from `ControlType::AgentObject`; emitted in the manifest.
  - Verify: manifest carries `"smb"` with an AgentObject and not without; `cargo tree`
    shows no `cobolt-ide`/`cobolt-agents` (AC13); same program under the run-form,
    child-form and compiled-binary paths.

- [ ] **T12 — Live test (operator step)** (AC4, AC6, AC8)
  - `#[ignore]` test driven by `COBOLT_TEST_SMB_URL`, `COBOLT_TEST_SMB_GUEST_URL`,
    `COBOLT_TEST_NET_PATH`: credentialed share, guest share, read-only share, OS network
    path. Run by the operator against a Samba container or their share; the result is
    reported, never assumed. A failing guest login is reported as a gap.

## Phase 5 — docs and finalize

- [ ] **T13 — System KB and Developer's Guide**
  - Files: `crates/cobolt-compiler/src/lib.rs` doc tables (methods, the five properties,
    refusal codes, the fallback, the project setting), `assets/knowledge/chunked.data`,
    `docs/developers-guide-en.md` (*Registering a file by path*).
  - Verify: `every_control_property_is_documented`,
    `prebuilt_chunked_kb_matches_the_published_documentation`; invalidated translations
    deleted (GOLDEN RULE #8).

- [ ] **T14 — Finalize**
  - Sweeps `--no-fail-fast`: `cobolt-runtime` (± `--features smb`), `cobolt-forms
    --features render`, `cobolt-form-host`, `cobolt-cli`, `cobolt-compiler`, `cobolt-ide
    --bin cobolt-ide`; AC14 numbers reported; spec ACs checked off; CHANGELOG + `z`.

## Acceptance-criteria coverage

| AC | Task | AC | Task |
|---|---|---|---|
| AC1 | T9 | AC8 | T9, T12 |
| AC2 | T6, T9 | AC9 | T4, T9 |
| AC3 | T9 | AC10 | T7, T9 |
| AC4 | T10, T12 | AC11 | T7, T9 |
| AC5 | T9, T10 | AC12 | T7, T9, T10 |
| AC6 | T5, T10, T12 | AC13 | T2, T11 |
| AC7 | every test | AC14 | every test, T14 |
