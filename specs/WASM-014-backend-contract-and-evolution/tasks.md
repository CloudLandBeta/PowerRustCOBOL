<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The backend contract and how it evolves (WASM-014)

- **Status:** draft. Written 2026-10-22 from `plan.md` (revision 2 of 2026-10-22). The server side and the Rust client are built (plan
  milestone M4). These tasks build milestones M5–M9: the wire alignment, sessions, the browser's side, the status group and the IDE/KB.
  - One task is **gated** on another spec: T15's end-to-end half needs WASM-010 T16 (the module lowers `CALL … WITH WEBSOCKET LINKAGE`).
  - Three working assumptions stay open for the operator (OQ1–OQ3, below).
- **Plan:** ./plan.md   **Date:** 2026-10-22

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s) it satisfies, and how to verify it. Check off
as completed.

**2.x.x overrides for this list.**
- There is **no documentation task**. `README.md` and everything under `docs/` are never touched on 2.x.x.
- What 2.x.x says about itself goes in `specs/`: the ABI amendment A10 (T6), the event rows (T9, T13, T14) and the `WASM-PROGRESS.md` row (T19).
- The user-facing description of the new built-ins, events, declarations and settings lives in the System KB text (T18).
- The finalize task (T20) bumps `z` once and writes one CHANGELOG entry.
- No task commits, pushes, checks out or switches branches.

**Grounding read for this list (2026-10-22, `features-2x` at 2.0.30, `cdad25cd`).** Every path below was opened for this list.
- **Wire — `crates/cobolt-wire/`.** Dependencies: `cobolt-ast`, `serde`, `serde_json` only.
  - `src/contract.rs`: `PROTOCOL` `:32`, `PROTOCOL_MIN` `:35` (both `1`); `ProgramSig` `:72` (`idempotent`, `independent`, `long_running`,
    `stream`, `deprecated`; **no `status`**); `InterfaceDescription` `:125`.
  - `src/proto.rs`: `Hello` `:30` (`protocol`, `contract`, `client`; **no `session`**, no `deny_unknown_fields`); `Call` (`key` optional);
    `Accepted` `:159`; `ServerHello` `:168`; `ClientFrame` `:324` (ten kinds, **no `Bye`**); `parse_client` `:340` refuses an unknown kind;
    `ServerFrame` `:376`; `parse_server` `:394`.
  - `src/error.rs`: `ErrorClass` `:19` (`ALL` has nine), `wire` `:55`, `word` `:70`, `from_wire` `:85`; `codes` `:92` (`1000`–`9002`,
    `APPLICATION_MAX = 7999`, `CANCELLED = 8000`, `PROGRAM_FAULT = 9001`); `CallError.may_have_run` `:144`.
  - `src/compat.rs`: `R_MISSING`, `R_TYPE`, `R_NARROWED`, `R_TABLE` `:37-40`.
  - `src/evolution.rs`: `Report` `:44` (`breaking` and `notes` are **`Vec<String>`**, not structured); `check_against_released` `:59`; six
    `#[test]`s.
  - `src/call.rs`: the `Transport` trait has `call` `:132`, `call_observed` `:135`, `upload` `:141`, `download` `:146`. It has **no subscribe,
    no hello and no version report**.
  - `tests/vectors.rs` reads language-neutral vectors from `specs/WASM-005-…/vectors/` (`:23`). That is the precedent for T7's vectors.
- **Server — `crates/cobolt-server/`.**
  - `src/serve.rs`:
    - `Shared.idem` `:90`, built at `:167` from `limits.idempotency_ttl` and `idempotency_max_entries`;
    - the per-connection permit `:532`; dispatch `ClientFrame::Hello` `:606`;
    - `negotiate` `:772` (a pure function, **no unit test exists**; its only caller is `on_hello` `:813`);
    - the caller key `subject` or `ip:<addr>` `:906`; the `long_running` ceiling `:925`;
    - the idempotency `begin`/`finish` path `:944-1082`.
  - `src/idem.rs`: `Store` `:54`, `fingerprint` `:66`, `Store::new(ttl, max)` `:77`, `begin(caller, key, fp)` `:89`, `finish` `:131`. It is
    in memory only.
  - `src/config.rs`:
    - every section is `deny_unknown_fields`; `LimitsSection` `:127` (`idempotency_ttl_seconds` `:149`, `idempotency_max_entries` `:161`);
      `FilesSection` `:166` (`dir`);
    - `ServerConfig` `:219` has `server`, `tls`, `auth`, `limits`, `files`, `identity`, and **no `[contract]`**;
    - the defaults `unwrap_or(600)` `:505` and `unwrap_or(10_000)` `:510`; `files.dir` defaults to `data/uploads` `:517`.
    - **There is no backend "state directory"**: `files.dir` is the only data path.
  - `src/backend.rs`: `ExportDecl` `:39` (no `status`), `DeprecatedDecl` `:63`, `Backend::build` `:154` (where the exports are checked against
    the parsed programs), `Backend::load` `:244`.
  - `src/client.rs`: `ClientConfig.reconnect_deadline` `:107`; `upload_state_lost` `:200`; `random_hex` `:204`; `WsClient::hello` `:234`,
    `subscribe` `:246` (an inherent method, not on the trait), `upload` `:269`, `download` `:343`; `Transport::call_observed` `:406`;
    `install_default_from_env` `:495`; the client's `Hello` built at `:576`.
  - `Cargo.toml`: `cobolt-runtime` (default-features off) is in the `server` and `client` features.
    `cobolt-runtime/Cargo.toml:185` pins `redb = "=4.3.0"` with `experimental-api-5`, **not optional**, and `:191` adds `redb2` (2.6). So
    redb 4.3.0 is already in the backend's graph.
  - Tests: `client_contract.rs` (13 tests, each named after its AC: `ac1_ac10_…` `:244`, `ac3_…` `:348`, `ac4_…` `:365`/`:408`, `ac6_…`
    `:431`/`:452`, `ac7_…` `:490`, `ac8_…` `:521`/`:541`, `ac9_…` `:610`/`:652`/`:677`; the `REJECT` program using `WS-FAIL` `:100`),
    `config_rules.rs`, `dependency_guards.rs` (four guards, none about redb), `loopback.rs`, `token_rules.rs`, `support/mod.rs`.
- **Runtime — `crates/cobolt-runtime/`.**
  - `src/interpreter/remote.rs`: `RemoteEvent` `:65` (`Progress`, `Warning` only); `set_default_transport_factory` `:77`; `set_remote_events`
    `:190`; `remote_cancel_handle` `:195`; the built-ins `WS-LAST-ERROR` `:442`, `WS-CLAIM` `:466`, `WS-FAIL` `:489`, `WS-PROGRESS` `:515`,
    `WS-CANCELLED` `:538`, `WS-PAUSE` `:547`, `WS-EMIT` `:568`, `WS-FILE-PATH` `:588`, `WS-FILE-PUBLISH` `:607`, `WS-WARNING` `:628`,
    `WS-UPLOAD` `:649`, `WS-DOWNLOAD` `:687`.
  - `src/interpreter.rs:11687-11695`: the `COBOL-WS-*` dispatch.
  - `src/builtins.rs`: `BUILTINS` `:35` (the `WS-*` rows `:95-103`).
  - `tests/test_backend_hooks.rs:298` is **the only caller of `set_remote_events`**. No form host wires `RemoteEvent`s today.
- **Compiler — `crates/cobolt-compiler/`.**
  - `src/backend_build.rs`: `BackendBuildOptions` `:37`, `BackendBuildResult` `:51`, `build_backend` `:92`; it writes `interface.json` `:124`
    and runs the check `:133`. A break fails unless `accept_breaking` or `debug`.
  - `src/runtime_features.rs`: `remote` `:142`, set by a WebSocket `CALL` `:290`.
  - `src/lib.rs`: the `WS-FAIL` doc row `:5105`; the KB section "Calling the backend — `CALL … WITH WEBSOCKET LINKAGE`" `:5130`.
  - `tests/backend_build_wasm003.rs` (`:98` a break refuses the build).
  - **`compiled_app_calls_backend`, which plan §6 cites, does not exist anywhere in `crates/`.**
- **CLI — `crates/cobolt-cli/src/main.rs`:** `cmd_backend` `:480`; the verbs `run|describe|check|release|build` `:511`; `check` runs
  `check_against_released` `:552`; `build` calls `build_backend` `:591`.
- **Project tools — `crates/cobolt-project-tools/src/create.rs`:** `backend/COPYBOOKS` `:71`; `BACKEND_STARTER` `:82`; the starter written at
  `:378`.
- **Player — `crates/cobolt-web-player/`.** It is a **separate workspace** (root `Cargo.toml:45` `exclude`). It depends on `cobolt-forms` and
  `cobolt-wasm-abi`, **not on `cobolt-wire`**, and has no WebSocket client off `wasm32`.
  - `src/net.rs`: the A8 envelope `:25`; `class::CLIENT` `:39`; `Config` `:70`; `error_frame(…, may_have_been_received)` `:137`;
    `{"type":"auth"}` `:198`/`:282`; `{"type":"call","id","frame"}` `:264`; `{"type":"ping"}` → `pong` `:311`; `reset_session` `:391`; tests
    asserting the class `client` `:601` and A8's `"expired":true` `:612`.
  - No file under `src/` names `localStorage`, `sessionStorage` or IndexedDB.
  - Tests: `abi_host.rs`, `dependency_guard.rs`, `freshness.rs`, `sinks.rs`.
- **Module and manifest.** `crates/cobolt-wasm-abi/src/manifest.rs`: `Manifest` `:92` (no `contract`, no interface). `crates/cobolt-wasm/src/`
  has no WebSocket-call lowering yet (WASM-010 T16 owns it). The ABI fixture's remote request is `{"program":"PING","payload":{}}`
  (`cobolt-wasm-abi/src/fixture.rs:1617`).
- **ABI — `specs/WASM-ABI-v1.md`:** A6 `:169`, A8 `:171`, A9 `:172`; **no A10**; the §12.2 event table (`onFilesDropped` `:215`).
- **IDE — `crates/cobolt-ide/`.**
  - The IDE links `cobolt-compiler` (`Cargo.toml:48`), **not `cobolt-server` or `cobolt-wire` directly**. **Nothing in the IDE calls
    `build_backend` or `check_against_released`.**
  - `src/panels/web_settings.rs`: the Backend part `:192`; `src/i18n.rs`: the `pk_set_*` keys from `:377`.
  - `src/grace_host.rs:4780` `prebuilt_chunked_kb_matches_the_published_documentation`; `src/version.rs:22` `VERSION = "2.0.30"`.
- **Sibling specs.**
  - WASM-003 `tasks.md` T5 adds `ServerHello.session: "new"` and calls it "the member WASM-014's protocol owns".
  - WASM-003 T9 edits the same Backend settings section; T10 runs `rcrun backend run` as a child of the IDE.
  - WASM-003 `tasks.md:209/243` and WASM-009 name WASM-014 as the owner of the envelope fix.
  - `specs/WASM-PROGRESS.md:43` has the WASM-014 row.

**Deviations from the plan, found while grounding.** Each is the smallest change that keeps the plan's intent.
- **Δ1 — `ServerHello.session`.** WASM-003 T5 adds `ServerHello.session: "new"` and defers its definition to this spec. T2 defines it:
  `"new" | "continued"`. It answers whether the store holds the session the client named. `memory` mode after a restart answers `"new"`, so the
  client knows its stored answers are gone. Whichever task lands first adds the member; the other reuses it.
- **Δ2 — where the `redb` file lives.** The plan says "under the backend's state directory", but there is none (`config.rs` has only
  `files.dir`). T3 adds `[contract] idempotency_path`, which defaults to `data/contract/idempotency.redb` and is resolved like `files.dir`
  (`config.rs:517`).
- **Δ3 — the `status` check's home.** Plan §3 puts it in `cobolt-semantic`. The analyser never sees `exports.toml`, and the exports are
  checked against the parsed programs in `cobolt-server/src/backend.rs` `Backend::build` `:154`. The check goes there (T11), and it reports
  through the same error list as the other export errors.
- **Δ4 — player ⇄ server testing.** The player is an excluded workspace with no native WebSocket, so no test can link both. Following
  WASM-013's Δ2:
  - a language-neutral transcript goes in `specs/WASM-014-backend-contract-and-evolution/vectors/wire.v1.json`, recorded from a real
    `cobolt-server` rig by a server test that fails when the rig's frames and the file differ;
  - `cobolt-wire` parses every frame of it;
  - the player's `net.rs` tests replay it (T7).
  - The live browser-to-backend run is T15, gated on WASM-010 T16.
- **Δ5 — the module's request frame.** Plan §2.8 aligns the envelope but does not name what the module hands to `remote`. Working assumption
  (T6), recorded in A10: the module's frame is the body of `proto::Call` **without** `t`, `id` and `key`, that is `{"program", "args",
  "returning"}`. The player adds `t`, `id` and `key`. WASM-010 T16 emits that shape. The fixture's `{"program","payload"}` is updated with it.
- **Δ6 — the missing tests the plan cites.**
  - `negotiate` has no table test (T1 writes one).
  - `compiled_app_calls_backend` does not exist (T13 writes it, `#[ignore]`, run with `--ignored`).
- **Δ7 — `RemoteEvent` reaches no host.** D-Progress and the version/warning events need a form host to install `set_remote_events`. T9 wires
  it in `cobolt-form-host` once, for `rcrun run-form`, embedded child forms and the compiled binary alike.
- **Δ8 — the IDE's compatibility report.** The IDE never runs the backend check, and `Report` holds English strings.
  - T16 adds a structured break `(program, parameter, item, rule)` to `evolution::Report`. The existing strings are kept as its `Display`, so
    `rcrun` output is unchanged.
  - It also adds a check-only `cobolt_compiler::backend_build::check_contract(project_dir)` that uses the loader alone: no cargo, no sockets.
    The IDE calls it on Build and writes the report through `Tr` keys.

**Open, carried and not decided** (plan §8). The working assumption is in brackets; none is decided by this list.
- **OQ1 (spec Q2, "to confirm"):** at-least-once for every call, or only for programs declared safe? [Only declared safe: R9 as written. T5
  and T6 repeat only `idempotent` programs. D-Repeat-All is not built. If the operator rules "every call", R9's text changes first, then T5/T6
  change.]
- **OQ2:** the default store mode. [`memory`.]
- **OQ3:** is the host-shown progress declaration (D-Progress) R13's "progress event the form can show"? [Yes.]
- **Δ5** and **Δ1's `"continued"` value** are working assumptions too, recorded in A10 and changeable there.

**Decided by the operator (binding, 2026-10-22).**
- **Q1:** both ways to report an application failure. Agents (Codex) use only `WS-FAIL`: T11, T12, T18.
- **Q3:** streams are in 2.0: T14, T15.
- **Q4:** downloads go through HTTPS: T15.
- **Q5:** retention is per session, nothing is kept in the frontend, and the store is in memory or on the server, configurable in the project
  properties: T3, T4, T5, T6, T17.

## M0 — the built part, measured

- [ ] **T1 — Baseline the built contract; add the `negotiate` table test** (R1–R3, R16; AC1 server half)
  - Files: `crates/cobolt-server/src/serve.rs` (a `#[cfg(test)]` table over `negotiate` `:772`).
  - Do:
    - Run `df -h .` first.
    - Run `cargo test -p cobolt-wire -p cobolt-server --no-fail-fast` and record every `test result` line here. Plan §9 is history, not a run.
    - Table rows: client 1.0 vs server 1.2 → `warn`, `update_to` 1.2. Major differs either way → `refused`, with an update message naming the
      side to update. Same major, client minor higher → `refused`. Protocol 0 and `PROTOCOL + 1` → `refused` with a text that a person can
      read.
  - Verify: `cargo test -p cobolt-server negotiate --no-fail-fast` prints each row (client, server, outcome, message). The baseline counts
    are recorded under this task.

## M5 — one wire, and sessions (plan §2.4, §2.8)

- [ ] **T2 — `hello.session`, `ServerHello.session`, the `bye` frame** (R1, R2, R10, R16; Q5)
  - Files:
    - `crates/cobolt-wire/src/proto.rs`:
      - `Hello.session: Option<String>` (`#[serde(default)]`; 32 lowercase hex);
      - `ServerHello.session: Option<SessionState>` (`new` | `continued`, Δ1);
      - `Bye {t}`; `ClientFrame::Bye`; `"bye"` in `parse_client`.
    - `crates/cobolt-wire/tests/vectors.rs`: round-trips the new members.
  - Do: protocol stays `1` (plan §2.1: protocol 1 is not frozen before 2.0.0). An old `hello` without `session` still parses. A malformed
    `session` (not 32 hex) is `validation 4001`.
  - Verify: `cargo test -p cobolt-wire --no-fail-fast` green, plus a test that a `hello` without `session` parses and one with `"session":"x"`
    is `BAD_MESSAGE`.

- [ ] **T3 — The `[contract]` section of `backend.toml`** (R10; Q5, OQ2, Δ2)
  - Files: `crates/cobolt-server/src/config.rs` (a `ContractSection`, `deny_unknown_fields, default`, holding `idempotency_store = "memory" |
    "server"` (default `memory`, OQ2), `session_idle_seconds` (default 900) and `idempotency_path` (default `data/contract/idempotency.redb`,
    resolved like `files.dir`); `limits.idempotency_ttl_seconds` becomes an optional cap with no default (`:505`)), and
    `crates/cobolt-server/tests/config_rules.rs`.
  - Do: a `backend.toml` written before this change starts unchanged (user code is sacred). A misspelt key in `[contract]` is refused with
    its name.
  - Verify: `cargo test -p cobolt-server --test config_rules --no-fail-fast`. The new tests are:
    - `an_existing_backend_toml_without_contract_still_starts`, with the fixture copied from today's scaffold;
    - `contract_section_reads_both_store_modes_and_refuses_an_unknown_key`.

- [ ] **T4 — The session store: keyed per session, two back ends** (R10; Q5)
  - Files:
    - `crates/cobolt-server/src/idem.rs`:
      - the key becomes `(caller, session, key)`;
      - `end_session(caller, session)`;
      - idle expiry by `session_idle_seconds`;
      - the optional TTL cap;
      - the `max` bound kept;
      - a `Backend` choice: `Memory`, or `Redb(path)`, which writes the stored outcome in one transaction before `finish` returns.
    - `crates/cobolt-server/Cargo.toml`: `redb = { version = "=4.3.0", features = ["experimental-api-5"], optional = true }` under `server`,
      the same pin and feature as `cobolt-runtime/Cargo.toml:185`.
    - `crates/cobolt-server/tests/dependency_guards.rs`: a new guard, `the_backend_links_exactly_one_redb_4`, which also prints the `redb2`
      2.6 shim it tolerates.
  - Do: only successes and application failures are stored (unchanged). One session never sees another's entries. Nothing is stored
    client-side.
  - Verify: `cargo test -p cobolt-server idem --no-fail-fast` and `--test dependency_guards`. The unit tests, each printing its counts:
    - `same_key_other_session_runs_again`;
    - `bye_ends_the_session_and_frees_its_entries`;
    - `idle_session_expires_after_the_configured_seconds`, with a fake clock;
    - `redb_store_survives_reopen_and_memory_store_does_not`.

- [ ] **T5 — The server and the Rust client speak sessions** (R2, R9, R10; Q2/OQ1, Q5)
  - Files:
    - `crates/cobolt-server/src/serve.rs`:
      - `on_hello` `:813` reads `session` and answers `ServerHello.session`;
      - `Bye` ends the session;
      - `begin`/`finish` `:944-1082` pass the session;
      - no session → today's per-connection behaviour, with the connection as the session.
    - `crates/cobolt-server/src/client.rs`:
      - `WsClient` makes one 128-bit session id at construction, in memory only;
      - it sends `session` in every `hello` (`:576`);
      - it sends `bye` on an orderly close and on sign-out.
    - `crates/cobolt-server/tests/client_contract.rs`.
  - Do: the repeat stays as built (`idempotent` only: OQ1's working assumption). A developer's repeat with the same key in the same session is
    replayed.
  - Verify: `cargo test -p cobolt-server --test client_contract --no-fail-fast`. All 13 existing tests stay green, plus four new ones:
    - `ac6_a_repeat_after_reconnect_in_the_same_session_is_replayed_not_run`, which prints the run count;
    - `ac6_a_new_session_does_not_see_another_sessions_answers`;
    - `ac6_server_store_survives_a_backend_restart_and_memory_store_answers_new`, which prints `ServerHello.session` in each mode;
    - `bye_frees_the_sessions_entries`.

- [x] **T6 — The player speaks `proto.rs`; ABI amendment A10** (R2, R3, R7, R9, R10, R16; AC1, AC4 player half; Δ5)
  - **Status (2026-10-26, work package FT-1, uncommitted, awaiting an independent judge):** built as below, with these deviations:
    - the ABI row is **A15** (A10 was taken by WASM-010 meanwhile); it supersedes A8 and A6's error object;
    - "the interface says idempotent" is read from the page's `config.json` `backend.idempotent` (and `backend.contract` for
      `hello`), which the Build fills from `backend/exports.toml` and `backend/generated/exports.toml`
      (`cobolt-project-tools` `web/generator.rs` `backend_contract`) — T8's manifest route is not built; an older page names
      contract `1.0.0` and repeats nothing;
    - `hello.session` is sent, but the server has no session yet (T2/Δ1): it ignores the member;
    - with no token the player sends no `auth` at all (`proto::Auth` requires a token);
    - the codes-equality check of T7 is a source read of `cobolt-wire/src/error.rs` (`net.rs`
      `the_players_codes_equal_cobolt_wire_error_codes`), not the transcript replay; T7 itself is still open;
    - an answer over the module's 64 KiB staging area fails `limit` 7000 "answer too large (limit 64 KiB)" in the player
      (`MODULE_ANSWER_LIMIT`, held equal to `cobolt-wasm-rt` `FRAME_SIZE` by a test); the limit is unchanged;
    - reconnection: back-off only while something waits (a dropped connection, a safe call waiting for its repeat); a failed
      attempt with nothing waiting stops, and the next call tries at once.
  - Run: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` — lib 275 passed, abi_host 22,
    dependency_guard 6, exposed 2, freshness 5, grid_calls 1, sinks 3; 0 failed. A real-browser run against `rcrun backend run`
    (no shim) is in the FT-1 snapshot of the PowerAnalyticsWeb parity job.
  - Files:
    - `crates/cobolt-web-player/src/net.rs`. On open it sends `{"t":"hello","protocol":1,"contract","client":"prc-wasm/<ver>","session"}`.
      It waits for `hello`, then sends `{"t":"auth"}`, then calls: `{"t":"call","id","key",…module frame}`, with a fresh `key` per call
      kept for its repeat. It also:
      - maps its own failures onto the closed list: `transport 1000/1001/1002`, `limit 7000`, `validation 4001`, and `may_have_run`;
      - drops the class `client`, `may_have_been_received`, the JSON `ping`/`pong` and the `type` envelope;
      - on `authentication 2002` refreshes the token and repeats once;
      - repeats a dropped call once only when the interface says `idempotent`;
      - keeps the player's own ids from `0x80000000`;
      - keeps the session id in a field of `Client`, never in browser storage.
    - `specs/WASM-ABI-v1.md`: row **A10**, superseding A6's error object and A8, with the module's request frame shape of Δ5.
    - `crates/cobolt-wasm-abi/src/fixture.rs:1617`: the request takes the Δ5 shape.
  - Do: since the player cannot link `cobolt-wire`, the error codes it emits are constants mirrored in `net.rs`. A test in T7 holds them
    equal to `cobolt_wire::error::codes`.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`. The rewritten `net.rs` tests replace `:474-659`:
    - the hello → auth → call order;
    - each failure mapped, printing the class and code per case;
    - `2002` refresh-and-repeat;
    - a safe program repeated once and an unsafe one failed with `may_have_run: true`.
    - `grep_has_no_client_class_or_may_have_been_received`: a source scan over `src/`, printing the hits (0).
    - `the_session_id_is_never_written_to_storage`: scans `src/` for `local_storage`/`session_storage`/`indexed_db`, and the mock socket sees
      the same session on a reconnect.

- [ ] **T7 — One transcript, both sides** (R2, R3, R7; AC1, AC4, AC10 halves; Δ4)
  - Files:
    - new `specs/WASM-014-backend-contract-and-evolution/vectors/wire.v1.json` (data, language-neutral, like WASM-005's vectors) and
      `vectors/README.md`;
    - `crates/cobolt-server/tests/client_contract.rs`: `wire_transcript_matches_the_rig`, which drives a rig through hello (ok/warn/refused),
      auth, a call, each error class, `bye`, and compares the frames with the file, ids and keys normalised;
    - `crates/cobolt-wire/tests/vectors.rs`: every server frame parses with `parse_server`, every client frame with `parse_client`;
    - `crates/cobolt-web-player/src/net.rs` tests: replay the server side of the transcript into `Client` and assert what it sends and the
      frames it hands the module.
  - Verify:
    - the three suites above are green, each printing how many frames it checked;
    - the codes-equality test lists every code the player emits next to `cobolt_wire::error::codes`;
    - after a deliberate edit of one frame in the JSON, the server test turns red (checked once and reverted).

## M6 — the frontend knows its contract (plan §2.1, §2.7)

- [ ] **T8 — The contract and the interface in the module's manifest** (R1; AC1)
  - Files:
    - `crates/cobolt-wasm-abi/src/manifest.rs`: `Manifest.contract: Option<String>` and `Manifest.remote: Vec<RemoteSig {name, idempotent,
      long_running, stream, independent}>`, both `#[serde(default)]`, so an older manifest loads;
    - `crates/cobolt-wasm/src/module.rs`: an `Options` field filled from `backend/contract/interface.json` when present;
    - `specs/WASM-ABI-v1.md` §10: the two members;
    - `crates/cobolt-web-player/src/net.rs`: `Config` takes them, and the `hello` sends `contract`.
  - Do: no interface file means `contract` is absent, and the player sends `"0.0.0"`, which the backend refuses with an update message.
  - Verify:
    - `cargo test -p cobolt-wasm-abi -p cobolt-wasm --no-fail-fast`: a manifest without the members still loads, and one with them
      round-trips;
    - the player suite: the `hello` carries the manifest's contract, and `idempotent` comes from `remote`.

- [ ] **T9 — `onBackendVersion` and `onBackendWarning` on the main form, in every host** (R2, R6; AC1, AC3; Δ7)
  - Files:
    - `crates/cobolt-runtime/src/interpreter/remote.rs`: `RemoteEvent::Version {state, message, update_to}`, and `Warning` carries the
      program;
    - `crates/cobolt-wire/src/call.rs`: a `CallObserver` hook for the hello outcome, with a default no-op;
    - `crates/cobolt-server/src/client.rs`: report each non-`ok` hello. After `refused`, every call fails `version 600x`;
    - `crates/cobolt-form-host/src/host.rs`: install `set_remote_events` on every interpreter it starts, the root form and embedded child
      forms, and raise the two events on the **main form** with the arguments of plan §2.7;
    - the player (`src/host.rs`, `src/api.rs`): the same two events from `net.rs`'s hello and replies;
    - `specs/WASM-ABI-v1.md` §12.2: two rows.
  - Verify:
    - `cargo test -p cobolt-runtime --test test_backend_hooks --no-fail-fast`;
    - `cargo test -p cobolt-form-host --no-fail-fast`: `version_warning_reaches_the_main_form_handler` and
      `deprecated_call_raises_on_backend_warning_from_an_embedded_child`, which print the argument received;
    - `cargo test -p cobolt-server --test client_contract ac3 --no-fail-fast`;
    - the player suite: a `warn` hello raises `onBackendVersion` with `warn⇥…⇥1.2.0`.

## M7 — the second way to report an application failure (plan §2.3, D-Status, Q1 decided)

- [ ] **T10 — `status` in the signature; rule `status-changed`** (R4, R5, R8; AC2)
  - Files:
    - `crates/cobolt-wire/src/contract.rs`: `ProgramSig.status: Option<String>`, `skip_serializing_if` none, so existing `interface.json`
      files are byte-identical;
    - `crates/cobolt-wire/src/compat.rs` or `evolution.rs`: `R_STATUS = "status-changed"`, raised when a status group is removed, renamed or
      reshaped. Adding one is a note.
  - Verify: `cargo test -p cobolt-wire --no-fail-fast` passes the six existing evolution tests plus `adding_a_status_group_is_compatible` and
    `removing_or_reshaping_a_status_group_breaks_with_status_changed`, which prints the break line.

- [ ] **T11 — `exports.toml` `status = "<name>"`; the loader checks it; the `WS-STATUS` copybook** (R8; Q1; Δ3)
  - Files:
    - `crates/cobolt-server/src/backend.rs`: `ExportDecl.status`. `Backend::build` `:154` refuses a `status` that names no group inside the
      program's `RETURNING` item, or a group whose layout is not `WS-STATUS` (code `9(4)`, message `X(200)`, detail `X(1000)`), naming the
      program and the item. It fills `ProgramSig.status`.
    - `crates/cobolt-project-tools/src/create.rs`: a Rust constant `WS_STATUS_COPYBOOK`, written to `backend/COPYBOOKS/WS-STATUS.cpy` by the
      backend scaffold (`:378`) with the generated-code banner, plus `[contract]` defaults in the scaffold's `backend.toml`.
  - Do: the scaffold never overwrites an existing `WS-STATUS.cpy` or `backend.toml` (user code is sacred). The copybook is regenerated only
    when missing. PRIME DIRECTIVE: the `.cpy` comes from a Rust constant, so no non-Rust source is committed.
  - Verify: `cargo test -p cobolt-server -p cobolt-project-tools --no-fail-fast`. The new tests are:
    - `status_naming_no_group_is_refused_with_program_and_item`;
    - `status_of_the_wrong_shape_is_refused`;
    - `scaffold_writes_ws_status_once_and_never_overwrites`;
    - the scaffolded backend still passes `rcrun backend check`.

- [ ] **T12 — The server answers a non-zero status like `WS-FAIL`** (R7, R8; AC5)
  - Files: `crates/cobolt-server/src/serve.rs` (after a normal return of a program with `status`: a code `1`–`7999` gives `application`, that
    code, the message, the detail and the caller's `BY REFERENCE` groups unchanged; a code outside that range gives `server 9001`, logged;
    `0` is success), `crates/cobolt-server/tests/client_contract.rs`.
  - Verify: `cargo test -p cobolt-server --test client_contract --no-fail-fast`:
    - `ac5_a_declared_status_group_arrives_in_on_exception_like_ws_fail`: `REJECT` via `WS-FAIL` and a `REJECT-S` via the status group give
      equal `CallError`s, and the parameters are unchanged; it prints both;
    - `ac5_a_status_code_out_of_range_is_a_server_fault`;
    - an idempotent status-group failure is stored and replayed (T4).

## M8 — progress, streams and files from a form (plan §2.5, §2.6, D-Progress)

- [ ] **T13 — `WS-SHOW-PROGRESS` in the interpreter hosts; the compiled-binary parity test** (R13; AC8; OQ3; Δ6, Δ7)
  - Files:
    - `crates/cobolt-runtime/src/interpreter/remote.rs`: `COBOL::"WS-SHOW-PROGRESS" ( progress-control [label-control [cancel-control]] )`
      records a one-shot declaration for the next long-running call;
    - `src/interpreter.rs:11687` (dispatch), `src/builtins.rs` (`BUILTINS` row);
    - `crates/cobolt-form-host/src/host.rs`: on `RemoteEvent::Progress` it sets the declared control's value and the label's text; a click on
      the cancel control sets `remote_cancel_handle`. No COBOL runs mid-call;
    - new `crates/cobolt-compiler/tests/compiled_app_calls_backend.rs` (`#[ignore]`): builds a small Web-backed desktop app, runs it against a
      rig, and checks one call, one progress declaration and one cancel.
  - Do: the declaration is spent when the call ends, whether it succeeded or failed. If it names a control that does not exist, the call
    fails with `validation`, with a message naming it.
  - Verify:
    - `cargo test -p cobolt-runtime -p cobolt-form-host --no-fail-fast`:
      - `progress_frames_reach_the_declared_controls`, which prints the values seen;
      - `cancel_control_ends_the_call_with_application_8000`;
      - the same test through an embedded child form;
    - `cargo test -p cobolt-compiler --test compiled_app_calls_backend -- --ignored`, recorded here. It needs a native build, so `df -h .`
      first.

- [ ] **T14 — Streams from COBOL: `WS-SUBSCRIBE`, `WS-UNSUBSCRIBE`, `WS-STREAM-ITEM`, `onStreamItem`, `onStreamEnd`** (R14; AC8; Q3)
  - Files:
    - `crates/cobolt-wire/src/call.rs`: `Transport::subscribe` / `unsubscribe` / `next_item`, defaulting to `not configured 1003`;
    - `crates/cobolt-server/src/client.rs`: `WsClient` implements them over the existing `subscribe` `:246`;
    - `crates/cobolt-runtime/src/interpreter/remote.rs`, `interpreter.rs`, `builtins.rs`: the three built-ins;
    - `crates/cobolt-form-host/src/host.rs`: raise `onStreamItem` (argument `sub-id`) once the instance's running task is done, and
      `onStreamEnd` (`sub-id⇥reason`); end the instance's subscriptions when it closes;
    - `specs/WASM-ABI-v1.md` §12.2: two rows.
  - Do: the closed inline-call vocabulary is not touched: these are `COBOL::"…"` built-ins. The restart flag reads `Y` on the first item after
    a re-subscription.
  - Verify: `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-server --no-fail-fast`:
    - `a_form_receives_stream_items_and_the_end`;
    - `a_reconnect_resubscribes_and_sets_the_restart_flag`;
    - `closing_the_form_unsubscribes`, which asserts that the server's subscription count is back to its baseline and prints it;
    - the existing `ac8_a_stream_…` (`client_contract.rs:541`) unchanged.

- [ ] **T15 — The player: progress, streams, upload in pieces, download by address** (R13, R14, R15; AC8, AC9, AC10; Q4)
  - Files: `crates/cobolt-web-player/src/net.rs` (`put-begin`/`put-chunk`/`put-end` with resume, `get-file`, `sub`/`unsub`/`event`,
    `progress`, `cancel`), `src/host.rs`, `src/api.rs` (D-Progress, the stream events, the picked-file handle as `WS-UPLOAD`'s `path`,
    `onFilesDropped` input; the download is the browser's own navigation to the issued `/dl/<token>`).
  - Do: hashing is incremental SHA-256 over `File.slice` pieces (`sha2` is already a native dependency, and it is added for `wasm32` if it is
    missing). The module can never choose a path. Nothing goes to browser storage.
  - Verify:
    - Player suite with a mock `File` and a mock socket:
      - `upload_3mb_in_pieces_resumes_after_a_cut_and_checksums`, which prints the pieces, the resume offset and the hash;
      - `a_dot_dot_name_is_refused_before_any_frame`;
      - `progress_frames_set_the_declared_controls`;
      - `cancel_sends_the_cancel_frame`;
      - `stream_items_raise_on_stream_item_after_the_task`;
      - `reconnect_resubscribes`.
    - The transcript of T7 grows the `put-*`, `sub` and `progress` frames, with the server side re-recorded.
    - **Gated (WASM-010 T16):** the AC10 end-to-end, a generated module plus the player plus a rig, in both skew directions (client 1.0 to
      backend 1.2, and client 1.2 to backend 1.1, which is refused). It is recorded here when the gate opens and listed as owed in T19 until
      then.

## M9 — the IDE and the System KB

- [ ] **T16 — Structured breaks; `check_contract`; the report in the Output panel** (R5; AC2; Δ8)
  - Files:
    - `crates/cobolt-wire/src/evolution.rs`: `Break {program, parameter, item, rule}` in `Report.breaks`; `breaking: Vec<String>` stays,
      produced from `Display`, so `rcrun` and `build_backend` print exactly what they print today;
    - `crates/cobolt-compiler/src/backend_build.rs`: `pub fn check_contract(project_dir) -> Result<Report, CompilerError>`, loader only.
      `build_backend` `:133` uses it;
    - `crates/cobolt-ide/src/app.rs`: Build on a project with a backend calls `check_contract` and writes a heading and one line per break
      to the Output panel. A release Build stops on a break, as `rcrun backend build` does;
    - `crates/cobolt-ide/src/i18n.rs`: the heading, "no incompatible change", "breaks the released contract {v}", and the hint to raise the
      major version, in all six languages. Program, item and rule stay as code identifiers.
  - Verify:
    - `cargo test -p cobolt-wire -p cobolt-compiler --no-fail-fast`: `backend_build_wasm003.rs:98` unchanged, plus
      `check_contract_reports_program_item_rule_without_cargo`;
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`: `no_empty_ui_translations`, plus `build_writes_the_contract_report_to_output`;
    - the AC2 run, once per R4 change, compatible and incompatible: `rcrun backend build --release` on a fixture prints the build outcome and,
      for a break, the program, the item and the rule. The table is recorded here.

- [ ] **T17 — The Backend settings: store mode and session idle time** (R10; Q5, OQ2)
  - Files:
    - `crates/cobolt-ide/src/panels/web_settings.rs` (the Backend part `:192`): a two-value choice (`memory`, `server`) and a seconds field
      that read and write `[contract]` of `backend/backend.toml`. The edit is surgical: other sections and comments are kept, and a file
      without `[contract]` gains only that section.
    - `crates/cobolt-ide/src/i18n.rs`: the labels and the two values, in all six languages.
  - Do: coordinate with WASM-003 T9, which edits the same section; whichever lands second rebases onto the other's rows.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`: `contract_settings_round_trip_and_keep_other_sections` (a byte diff of
    the untouched sections prints empty) and `no_empty_ui_translations`. Manual check (the operator's): the fields at the default window size
    in all six languages, with no overflow (the egui-resize memory).

- [ ] **T18 — System KB: the contract, as the agents must write it** (R6–R15 as language behaviour)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`: the built-in rows (`:5105` neighbourhood) for `WS-SHOW-PROGRESS`, `WS-SUBSCRIBE`,
      `WS-UNSUBSCRIBE` and `WS-STREAM-ITEM`.
    - The same file, the section "Calling the backend" `:5130`, which gains:
      - the sessions and the delivery guarantee, stated as the working assumption (OQ1) and changed when the operator rules;
      - `onBackendVersion` and `onBackendWarning`; `onStreamItem` and `onStreamEnd`;
      - the `status` declaration and `status-changed`;
      - `[contract]` and the idle expiry;
      - **"generated and agent-written COBOL reports a business failure with `WS-FAIL` only; the status group is for hand-written and
        existing COBOL."**
    - `crates/cobolt-runtime/src/builtins.rs` `BUILTINS`; the regenerated `assets/knowledge/chunked.data`.
  - Do: the COBOL examples follow the extensions ruling and the call-comment ruling. They are checked with `rcrun check` on a scratch project
    outside the tree.
  - Verify:
    - `cargo run -p cobolt-ide --example build_chunked_kb`, then `cargo test -p cobolt-ide --bin cobolt-ide
      prebuilt_chunked_kb_matches_the_published_documentation`, green after the regeneration (it is red before it);
    - `cargo test -p cobolt-runtime builtins --no-fail-fast`.

- [ ] **T19 — The spec ledger** (all; specs only)
  - Files: `specs/WASM-PROGRESS.md` (the WASM-014 row `:43`: M5–M9 state, each AC's evidence, the owed rows: T15's gated end-to-end; no run
    on Windows, Linux or behind a proxy; OQ1–OQ3 open), and this file's checkboxes and recorded counts.
  - Verify: every AC1–AC10 cites a test name or an owed row, and `git status` shows nothing under `docs/` and no `README.md` change.

- [ ] **T19b — D21: the key exchange in the handshake** (R18; AC11) — added 2026-10-24 by the operator's ruling.
- [ ] **T20 — Finalize: version, CHANGELOG, the tests of the touched crates**
  - Files:
    - `crates/cobolt-ide/src/version.rs`: `z` + 1, **once**. Go from `2.0.30` to the next free `2.0.z` at that time; never change `x` or `y`.
    - `CHANGELOG.md`: **one** entry above the previous `## [PowerRustCOBOL 2.0.z]` heading, dated, "Written on features-2x". It lists:
      - the aligned wire and A10, with the player's error classes changed (`client` and `may_have_been_received` are gone);
      - sessions and `[contract]`;
      - the status group;
      - the new built-ins and events;
      - the IDE report;
      - what stays open: OQ1–OQ3 and T15's gated end-to-end.
  - Do:
    - Run `df -h .` first.
    - Run the tests of the crates touched, with `--no-fail-fast`, and read every `test result` line:
      - `cargo test -p cobolt-wire -p cobolt-server -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler -p cobolt-project-tools -p
        cobolt-wasm-abi -p cobolt-wasm -p cobolt-cli --no-fail-fast`;
      - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
      - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`;
      - `cargo test -p cobolt-compiler --test compiled_app_calls_backend -- --ignored`.
  - Verify:
    - Everything above is green, and the counts are recorded here.
    - The staged diff has no credential: no `AKIA`/`ASIA`, no PEM block, no token outside the test fixtures.
    - `git status` shows nothing under `docs/` and no `README.md` change.
    - Manual check (the operator's): launch the IDE, confirm the version string, run Build with a deliberate break, and read the report in
      two languages.

## Done criteria
- Every acceptance criterion in spec.md is covered by at least one task's verification:
  - **AC1** (version skew, refusal with an update message): T1, T6, T7, T8, T9, plus the existing `ac1_ac10_…`;
  - **AC2** (compatible changes build, breaks fail naming program/item/rule): T10, T16, plus `backend_build_wasm003.rs`;
  - **AC3** (deprecated works, warning event): T9, plus the existing `ac3_…`;
  - **AC4** (each class produced and read; unknown → `server`): T6, T7, plus the existing `ac4_…`;
  - **AC5** (business failure in `ON EXCEPTION` with code and data): T11, T12, plus the existing `REJECT` case;
  - **AC6** (safe repeat, `may_have_run`, key replay): T4, T5, T6;
  - **AC7** (ordering): the existing `ac7_…`, re-run in T1 and T20;
  - **AC8** (progress, cancel, streams): T13, T14, T15;
  - **AC9** (files in pieces, resume, checksum, `../`, expired address): T15, plus the existing `ac9_…`;
  - **AC10** (both skew directions, end to end): T7 (the halves) and T15 (gated on WASM-010 T16; an owed row in T19 until then).
- The tests pass, the System KB is regenerated, and the i18n test is green in six languages.
- No `README.md` or `docs/` change. The change is a **feature** on `features-2x`. Nothing is committed or pushed by these tasks.
