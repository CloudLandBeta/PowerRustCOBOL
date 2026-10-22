<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The Backend project and the secure frontend–backend channel (WASM-003)

- **Status:** draft — the 2026-10-20 work (M3) is done and kept below with its evidence; this list covers what the plan revised on
  2026-10-22 adds (plan §0 C1–C5) and the two acceptance halves still open (AC9 IDE half, AC10). Nothing in T1–T13 is implemented.
- **Plan:** ./plan.md   **Date:** 2026-10-22

Ordered so the tree stays green after each task: the runtime prerequisite first (it can stop the `per-connection` work), then the single
exports list and the property (data before behaviour), then the server work, then the Leptos front (dependency guard before code), then the
IDE, then the cross-checks and the finalize. 2.x.x line: **no documentation task** — `README.md` and `docs/` are never touched; what 2.x.x says
about itself stays in `specs/`.

Grounding read for this list (path and symbol):
`crates/cobolt-server/Cargo.toml` (features `loader`, `server`, `client`; `httparse` optional `:83`), `src/http.rs` (`read_head` `:54`,
`respond` `:135`), `src/serve.rs` (`health_loop` `:256`, `serve_stream` `:309`, Origin check `:335`, `WebSocketConfig` `:344`, `serve_download`
`:364`, `origin_allowed` `:423`, `session` `:496`, `on_text` `:597`, `on_hello` `:813`, `on_call` `:840`), `src/exec.rs` (`execute` `:81` builds a
new `Interpreter` every call, `:92`), `src/backend.rs` (`ExportDecl` `:39` with `idempotent`/`independent`, `Backend::load` `:244`),
`src/config.rs` (`deny_unknown_fields`; `unsafe_dev_token_secret` `:229`, `unsafe_allow_any_origin` `:231`, `unsafe_plain_ws` `:233`, its warning
`:416`), `src/bundle.rs` (`Bundle` `:24`, no `state`), `src/dev.rs` (`DevServer` `:42`, `start`, `log_lines`, `stop`); `tests/loopback.rs`,
`config_rules.rs`, `dependency_guards.rs` (4 tests), `token_rules.rs`, `client_contract.rs`; `crates/cobolt-wire/src/proto.rs` (`Hello` `:30`,
`ServerHello` `:168`, `parse_client` `:340`); `crates/cobolt-runtime/src/interpreter.rs` (`with_external_store` `:2056`, `set_file_lock_wait`
`:2786`, `run` `:4583`, `external_store` `:4638`), `interpreter/remote.rs` (`set_remote_claims` `:221`, `group_from_json` `:246`),
`interpreter/exec_sql.rs` (`sql_end` `:158`), `builtins.rs` (`WS-CLAIM` `:93`, already in `BUILTINS`);
`crates/cobolt-compiler/src/project_kind.rs` (`BackendSettings` `:376`, only `exports: Vec<String>`), `src/lib.rs` (`ManifestView` `:1087`, field
`backend_exports` `:1110` filled from `[backend] exports` `:1163`; System KB section *Calling the backend* `:5130`, "Every call runs in a fresh
interpreter" `:5154`), `src/backend_build.rs` (`build_backend` `:92`, `host_triple` `:105`); `crates/cobolt-project-tools/src/create.rs`
(`BACKEND_STARTER` `:82`, `starter_backend` `:148`, writes `[backend] exports = ["HEALTH"]` into `cobolt.toml` `:531`, no `exports.toml`, no
`backend.toml`); `crates/cobolt-ide/src/panels/web_settings.rs` (`parse_exports` `:79`, writes `backend_settings_mut().exports` `:193`, the
exports field `:357`); `crates/cobolt-ide/src/i18n.rs` (`pk_set_exports_hint` `:407`, six language tables); `crates/cobolt-ide/src/form_runtime.rs`
(`sibling_rcrun` `:120`, the child-process `rcrun run-form` pattern `:443`); `crates/cobolt-ide/Cargo.toml` (no `cobolt-server` dependency; no
`cobolt_server` use anywhere in `crates/cobolt-ide/src`); `crates/cobolt-cli/src/main.rs` (`cmd_backend` `:480`, verbs `run|describe|check|release|build`);
`crates/cobolt-web-bundle/src/secrets.rs` (`scan_text` `:197`, `is_text_path` `:257`; no mention of `backend/`);
`crates/cobolt-web-player/src/net.rs` (`{"type":"call",…}` `:264`); `crates/cobolt-ide/src/version.rs` (`VERSION = "2.0.29"` `:22`);
`specs/steering/tech.md:64-74` (System KB rule, `cargo run -p cobolt-ide --example build_chunked_kb`).

## Done (2026-10-20, kept with their evidence — plan §9)

- [x] **D-a — The backend crate, loader and headless build** (R1, R2 minus Leptos, R3, R4; AC1): `crates/cobolt-server` (`backend.rs`,
  `bundle.rs`, `standalone.rs`), `crates/cobolt-compiler/src/backend_build.rs` (host triple only, Q6), `rcrun backend …` (`cmd_backend`).
  `backend_build_wasm003` 3 pass; built in 29 s, 53.1 MB.
- [x] **D-b — The channel, tokens, hardening** (R5–R17, R21; AC1–AC8): `serve.rs`, `jwt.rs`, `jwks.rs`, `limits.rs`, `log.rs`, `config.rs`;
  `loopback` 11, `token_rules` 15, `config_rules` 10, `dependency_guards` 4 pass. Development token in a development run only (Q5,
  `a_release_start_refuses_every_unsafe_setting_and_a_development_start_only_warns`).
- [x] **D-c — Cross-connection file locking** (R19): `Interpreter::set_file_lock_wait`; `calls_that_share_a_file_take_turns_and_none_is_lost`
  (24 writers, all status 00). A runtime **fix** by the 2026-09-14 ruling, forward-ported.
- [x] **D-d — The claims built-in** (R12; AC3): `COBOL::"WS-CLAIM"` in `BUILTINS` and in the KB section.
- [x] **D-e — CLI and development server** (R20 standalone half; AC9 partial): `backend_cli` 3 pass; `DevServer` ready for the IDE.

## The runtime prerequisite (plan §2.6, §5 R3)

- [ ] **T1 — One interpreter can run a program twice and keep `WORKING-STORAGE`; it can cross threads** (R18, Q2)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (test module only, unless the test fails).
  - Do: unit test `interpreter_reentry_keeps_working_storage` — one `Interpreter`, a counter program, `run()` twice: answers 1 then 2; with
    the `sql` feature, `sql_end` has run at the end of each `run` (no transaction carried across). A compile-time assertion that `Interpreter`
    is `Send` (`fn assert_send<T: Send>() {}`), which `spawn_blocking` needs.
  - **If either fails, stop here:** the fix is a separate runtime task with interpreter/binary parity (`rcrun run-form`, embedded child forms,
    the compiled binary, the backend — `.claude/skills/interpreter-binary-parity`), recorded in this list before T5 starts. No workaround
    inside `cobolt-server`.
  - Verify: `cargo test -p cobolt-runtime interpreter_reentry --no-fail-fast` and `cargo test -p cobolt-runtime --features sql interpreter_reentry`;
    the test prints the two answers.

## One list of exports, one property (plan C2, C5, §3; D5)

- [ ] **T2 — The scaffold writes `backend/exports.toml` and a development `backend/backend.toml`** (R1, R22; C5)
  - Files: `crates/cobolt-project-tools/src/create.rs` (beside `BACKEND_STARTER` and `starter_backend`; the `[backend]` table write `:531`).
  - Do: a new Web or Mobile project gets `backend/exports.toml` exporting `HEALTH` with `public = true`, and a `backend/backend.toml` that
    listens on loopback, has no secret and no `unsafe_*` setting. Both carry the generated-file comment the other scaffold files carry (they
    are scaffold, written once — the developer's thereafter; never rewritten over an existing file). The `[backend] exports` line in
    `cobolt.toml` is no longer written (D5); `state` is not written either (absent = `per-call`, T3). The WASM-009 authentication programs are
    **not** added here (plan §2.1: M5 adds them when they exist).
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` — a new test creates a Web project in a temp folder and asserts both files,
    then runs `rcrun backend check <dir>` on it (exit 0, prints the count of exports); an existing `exports.toml` is left byte-identical.

- [ ] **T3 — `[backend] state`, and `exports` retired as a source** (R18, Q2 — *operator*; C2, C5)
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`BackendSettings`), `crates/cobolt-compiler/src/lib.rs` (`ManifestView::backend_exports`
    re-pointed at `backend/exports.toml` through the `cobolt-server` loader the compiler already links), `crates/cobolt-server/src/bundle.rs`
    (`Bundle` gains `state`), `crates/cobolt-compiler/src/backend_build.rs` (copies `state` into the bundle), `crates/cobolt-cli/src/main.rs`
    (`cmd_backend` `run` reads `[backend] state` from the project's `cobolt.toml`).
  - Do: `state: String` with `#[serde(default)]` → `"per-call"`; accepted values `per-call` | `per-connection` (**O2:** the plan's spelling;
    the visible words are Tr keys, T9). Any other value is a Check/Build error naming the two values. `exports` still deserialises (old
    manifests load) but nothing reads it as the list. An absent `[backend]` table or `state` = `per-call`, so every existing project is unchanged.
  - Verify: `cargo test -p cobolt-compiler --no-fail-fast` (a round-trip test: absent → `per-call`; `per-connection` survives a save; `"session"`
    is refused with the message) and `cargo test -p cobolt-server --no-fail-fast` (bundle round-trip carries `state`).

## The server (plan §2.2–§2.6; D3, D4)

- [ ] **T4 — `unsafe_plain_ws` is removed** (R5, Q7 — *operator*; D4; AC4)
  - Files: `crates/cobolt-server/src/config.rs` (the field `:233`, its warning `:416`, the `unsafe_set` list), `crates/cobolt-server/tests/config_rules.rs`.
  - Do: plain `ws` is served only on a loopback address, in every start mode (that is the reverse-proxy arrangement of Q7). A `backend.toml`
    still naming `unsafe_plain_ws` fails to start with a specific message ("`unsafe_plain_ws` was removed: plain ws:// is served on loopback only;
    use TLS …"), not the generic unknown-field one. *Reading recorded (plan D4), not decided here:* R5's wording should say "loopback `ws` also in
    a release start, behind a reverse proxy" at the next `/clarify`.
  - Verify: `cargo test -p cobolt-server --test config_rules --no-fail-fast` — `plain_ws_is_never_served_on_a_non_loopback_address` replaces the
    `unsafe_plain_ws` case of `plain_ws_is_for_loopback_only_and_tls_is_all_or_nothing`, and reports each address tried and its outcome;
    `cargo test -p cobolt-server --test loopback a_foreign_origin` still passes.

- [ ] **T5 — `per-connection` sessions** (R18, R19, Q2 — *operator*, Q4; D3) — *requires T1 green and T3*
  - Files: `crates/cobolt-server/src/exec.rs` (a session type holding one `Interpreter` per exported program called, sharing one `ExternalStore`
    via `Interpreter::with_external_store`), `crates/cobolt-server/src/serve.rs` (`session`, `on_call`: pick `execute` or the session by the
    bundle's `state`; claims re-installed at every call with `set_remote_claims`; a call whose token names another `sub` than the session's
    first is refused `authorisation`; the per-connection concurrency permit is 1 in this mode), `crates/cobolt-server/src/log.rs` (the record
    gains the mode, not its contents), `crates/cobolt-wire/src/proto.rs` (`ServerHello` gains `session: "new"` — the member WASM-014's
    protocol owns; added as that spec defines it, nothing else in the envelope changes), `crates/cobolt-compiler/src/backend_build.rs` (a
    warning when an export says `independent` in `per-connection` mode, since it is ignored).
  - Do: a dropped connection drops its session (files closed, locks released); a reconnection starts fresh — nothing resumed, so R7 and Q4
    (nothing safe to repeat by default) are untouched. `per-call` stays today's `execute`, byte for byte.
  - Verify: `cargo test -p cobolt-server --test loopback --no-fail-fast` —
    `per_connection_state_is_kept_and_never_shared` (two connections, interleaved counter calls: each sees 1, 2, 3; a reconnect starts at 1; a
    token of another subject is refused; `per-call` answers 1 every time; the sequences printed) and
    `a_file_held_open_across_calls_makes_others_wait_or_93` (another connection gets status 93 within the configured wait, never hangs; the
    wait measured and printed); `concurrent_callers_get_their_own_results_and_no_state_survives_a_call` unchanged (AC7);
    `cargo test -p cobolt-wire --no-fail-fast`.

## The Leptos front (plan C1, C3; D1, D2, D6; §5 R2, R5) — O1 working assumption

**O1 is open** (WASM-003 R2 names `cargo-leptos`; WASM-017 R17 keeps `cargo-leptos` in the product's build only). **Working assumption, not
decided:** the backend is built on the Leptos framework inside `cobolt-server` with plain `cargo build`; `cargo-leptos` is not needed on the
developer's machine. If the operator rules otherwise, T7 gains a tool check with a missing-tool message (Tr ×6) and WASM-017 R17 needs an amendment.

- [ ] **T6 — The Leptos server crates enter behind `server`, guarded first** (R2; plan §5 R2)
  - Files: root `Cargo.toml` (`[workspace.dependencies]`: `leptos`, `leptos_axum`, `axum`, `hyper`, `hyper-util`, `default-features = false`,
    server side `ssr` only), `crates/cobolt-server/Cargo.toml` (`server` feature names them; `loader` and `client` do not),
    `crates/cobolt-server/tests/dependency_guards.rs`.
  - Do: add the dependencies and nothing that uses them yet. Extend the guards: no `ring`, `aws-lc-*`, `rustls*`, `openssl*` in the `server`
    closure; no `leptos*`, `axum`, `hyper*` in the `loader` closure (`the_compiler_reads_and_packs_a_backend_through_the_loader_alone`) nor in
    the `client` closure. Check each added crate's `rust-version` ≤ 1.92. **If a barred crate or a newer MSRV is unavoidable, stop and report;
    the guard is not weakened.**
  - Verify: `cargo test -p cobolt-server --test dependency_guards --no-fail-fast` (each guard prints the closure size and the barred names it
    found: none); `cargo check -p cobolt-server --no-default-features --features loader`.

- [ ] **T7 — The HTTP front moves onto the Leptos/axum router; the session code does not change** (R2, R5, R8, R14, R16, Q3 — *operator*; D1, D6)
  - Files: `crates/cobolt-server/src/serve.rs` (the accept loop keeps the connection limit and the TLS handshake timeout and hands the
    `native-tls` stream to `hyper_util`'s connection server with upgrades; a closed route table built through `leptos_axum`: `GET /health`,
    `GET /dl/<token>` → `serve_download`, `GET /` and `GET /ws` with an upgrade, everything else 404; the upgrade route runs `origin_allowed`,
    then `tokio_tungstenite::WebSocketStream::from_raw_socket` with today's `WebSocketConfig`, then `session` unchanged; `health_loop` becomes
    a second listener on the same router), `crates/cobolt-server/src/http.rs` (`read_head` and `respond` retire; the security headers move
    to a router layer), `crates/cobolt-server/Cargo.toml` (`httparse` dropped), a mount point the WASM-009 identity routes use later (no route
    added for them here).
  - Do: `on_hello` / `on_text` / `on_call` and `jwt.rs` untouched (D1). A developer cannot add a route (D6). The generated backend project
    names no Leptos crate (D2, R2 "invisible").
  - Verify: `cargo test -p cobolt-server --no-fail-fast` — every 2026-10-20 test passes unchanged on the new front (counts read from every
    `test result` line), plus `loopback::routes_are_closed` (each path and its status printed) and
    `loopback::a_large_or_slow_request_head_is_refused` (over-cap header → 431; a head that never ends → closed within the handshake timeout,
    the time printed). `grep -n "httparse" crates/cobolt-server/Cargo.toml` finds nothing.

- [ ] **T8 — The built binary on the new front** (R2, R3, R4, Q6 — *operator*; AC1)
  - Files: `crates/cobolt-compiler/tests/backend_build_wasm003.rs` (assertions added), `crates/cobolt-compiler/src/backend_build.rs` only if
    the bundle/`state` change needs it.
  - Do: build a backend with one exported and one non-exported program; the staging `Cargo.toml` names `cobolt-server` and no `leptos*`/`axum`
    crate; the binary answers `/health`, `--describe` prints `interface.json`, the non-exported program is `not-found 5000`.
  - Verify: `cargo test -p cobolt-compiler --test backend_build_wasm003 --no-fail-fast`; the test prints the binary size and the cold build time,
    recorded in T12 against 53.1 MB / 29 s (plan §5 R5). `cargo test -p cobolt-cli --test backend_cli --no-fail-fast` (3) still green.

## The IDE (plan C5, §3; R20; AC9)

- [ ] **T9 — The Backend settings section edits `backend/exports.toml` and `state`; old projects migrate once** (R1, R18, R20; C2, C5; O2)
  - Files: `crates/cobolt-ide/src/panels/web_settings.rs` (the exports field `:357` reads and writes `backend/exports.toml` — names added
    as `[[export]] program = …`, an existing entry's `roles`/`public`/`idempotent`/… preserved, never dropped; a `state` choice of two values),
    the project-open path in `crates/cobolt-ide/src/app.rs` (the migration), `crates/cobolt-ide/src/i18n.rs`.
  - Do: on open, `backend/exports.toml` absent and `[backend] exports` non-empty → write `exports.toml` from the names (no roles, not public)
    and say so in the Output panel; `cobolt.toml` untouched (user code is sacred). New `Tr` keys in **all six** languages: the `state` label,
    its two values (O2: the visible words are the operator's to change), the migration notice; `pk_set_exports_hint` re-worded to name the file.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — `opening_a_project_with_old_exports_writes_exports_toml_and_leaves_cobolt_toml`
    (asserts the new file and `cobolt.toml` byte-identical), an edit round-trip that keeps an entry's `roles`, and the i18n test (no empty
    translation in the six tables). Window check per the egui-resize memory: the section at its default size in six languages.

- [ ] **T10 — Start and stop the backend from the IDE, its log in the Output panel** (R20; AC9 IDE half) — *design not in the plan*
  - The plan lists this as not done (§7) and names `cobolt_server::dev::DevServer` as ready, but does not choose how the IDE hosts it; the IDE
    links no `cobolt-server` today. **Working assumption, to confirm by the operator before this task starts:** the IDE runs
    `rcrun backend run <project>` as a child process (the `sibling_rcrun` pattern of `form_runtime.rs:120`), streams its stdout/stderr into the
    Output panel and kills it on Stop — so the IDE runs exactly the standalone code (AC9's "behaves identically") and the IDE links no
    socket/TLS/token crate. The alternative (linking `DevServer` in-process) would add the server closure to the IDE and needs the guard of T6
    extended.
  - Files: `crates/cobolt-ide/src/app.rs` (Start/Stop actions, the child handle, stopped on IDE exit), the Backend menu/toolbar entry WASM-001
    defines, `crates/cobolt-ide/src/i18n.rs` (Start, Stop, "backend running on …", "backend stopped", a start failure — six languages).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — a test starts the child on a temp Web project, reads `GET /health`
    200 and the address line in the captured log, stops it, and asserts the process is gone and the port free (timings printed). Manual
    (operator, never driven by the agent): Start in the IDE, `curl -k https://127.0.0.1:<port>/health`, Stop.

## Cross-checks (R12, R17, R23; AC10)

- [ ] **T11 — System KB: *Calling the backend* rewritten; `chunked.data` regenerated** (R12, R18, R19; Q2)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the section at `:5130`; the sentence at `:5154` "Every call runs in a fresh interpreter"
    becomes the `state` property: `per-call` default, `per-connection` keeps `WORKING-STORAGE` per connection, serialised, never resumed
    across connections, a file left open holds its lock across that connection's calls; `backend/exports.toml` is the one list),
    `assets/knowledge/chunked.data` (regenerated).
  - Do: `cargo run -p cobolt-ide --example build_chunked_kb` in the same change. `WS-CLAIM` is already in `BUILTINS`; no new built-in.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`; `git diff --stat` shows
    `chunked.data` changed.

- [ ] **T12 — AC10 and R23: a secret stops the frontend build, the same value in `backend.toml` does not; `connect-src` printed**
  (R17, R23; AC10) — *the scan itself is WASM-002's (T2 there, done)*
  - Files: `crates/cobolt-web-bundle/tests/backend_secret_not_scanned.rs` (new, test only).
  - Do: one project fixture: a planted test secret in a form stops the bundle (naming file and pattern, never the value); the same value only
    in `backend/backend.toml` leaves the bundle building, and no file under `backend/` is in the bundle; the build report names the
    `connect-src` (`csp.rs`) for the configured backend address.
  - Verify: `cargo test -p cobolt-web-bundle --test backend_secret_not_scanned --no-fail-fast` — prints the two outcomes and the `connect-src`.

## Evidence and finalize

- [ ] **T13 — Evidence re-run on the Leptos front, recorded here** (AC1–AC10; plan §5 R5, R7)
  - Do: `df -h .` first (a test sweep links many binaries). Re-run every suite the plan §6 names and append an **Evidence (re-run)** table to
    this file: one row per acceptance criterion, the test that proves it, the printed numbers; AC9 and AC10 rows from T10 and T12. State that
    Windows and Linux were not run, if they were not, and that no real identity provider was contacted. Cross-spec risk R1 (the player's
    `{"type":…}` envelope vs `proto.rs` `{"t":…}` with `hello`, `net.rs:264` vs `parse_client`) is recorded, not fixed — WASM-014/WASM-016 own it.

- [ ] **T14 — Finalize** (all)
  - Files: `crates/cobolt-ide/src/version.rs` (bump `z` **once**: `2.0.29` → the next free `2.0.z` at the time), `CHANGELOG.md` (one entry
    **above** the previous heading, `## [PowerRustCOBOL 2.0.z] — <date>`, marked *Written on features-2x*, listing T1–T13). **No documentation
    task:** `README.md` and `docs/` are not touched on 2.x.x.
  - Verify: the tests of every crate touched, with `--no-fail-fast`, every `test result` line read:
    `cargo test -p cobolt-runtime --no-fail-fast`, `cargo test -p cobolt-wire --no-fail-fast`, `cargo test -p cobolt-server --no-fail-fast`,
    `cargo test -p cobolt-compiler --no-fail-fast`, `cargo test -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-project-tools --no-fail-fast`,
    `cargo test -p cobolt-web-bundle --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo fmt --check`;
    the credential scan of the staged diff (no `AKIA`/`ASIA`/secret) before the pipeline commits.

## Coverage (acceptance criteria → tasks)

| AC | Covered by |
|---|---|
| AC1 exported vs non-exported, one executable | D-a, T7 (re-run), T8 |
| AC2 token refusals | D-b, T7 (re-run unchanged) |
| AC3 role before start, subject readable | D-b, D-d, T5 (claims re-installed per call), T7 |
| AC4 foreign Origin, non-loopback `ws` | T4, T7 |
| AC5 payload validation | D-b, T7 |
| AC6 size, time, rate | D-b, T7 |
| AC7 concurrent clients isolated | D-b, T5 (`per-call` unchanged, `per-connection` per connection) |
| AC8 no token/password in the log | D-b, T5 (log gains only the mode), T7 |
| AC9 IDE start/stop, binary identical | D-e, T10 (*working assumption to confirm*) |
| AC10 frontend secret stops build, backend's does not | T12 |

## Open items carried (not decided by this list)

- **O1** — how `cargo-leptos` enters (R2 vs WASM-017 R17): working assumption above T6; to confirm.
- **O2** — the words `per-call` / `per-connection`: plan's spelling; the visible text is in Tr keys (T9).
- **T10's hosting model** — child `rcrun` process vs in-process `DevServer`: working assumption stated; to confirm.
- **D4 reading of R5** — loopback `ws` in a release start behind a reverse proxy; for the next `/clarify`.
- **O3** — WASM-INDEX D4/D5 still say "Open"; another file, left to its owner.
- **Plan §5 R1** — the player/server envelope mismatch; WASM-014/WASM-016.

## Done criteria
All acceptance criteria in spec.md are checked with the evidence of T13, the tests of every touched crate pass, the System KB and
`chunked.data` agree, the six i18n tables have the new keys, and the version and CHANGELOG are updated once. Commits and pushes are the
pipeline's, not this list's.
