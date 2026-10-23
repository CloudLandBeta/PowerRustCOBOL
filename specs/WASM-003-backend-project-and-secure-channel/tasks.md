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

- [x] **T1 — One interpreter can run a program twice and keep `WORKING-STORAGE`; it can cross threads** (R18, Q2)
  - *Result (2026-10-23):* re-entry held (`interpreter_reentry_keeps_working_storage`: `["0001", "0002"]`, no SQL session left after
    either run); `Send` **failed** — `OpenFile::Indexed(Box<dyn IndexedStore>)` was not `Send`. Recorded as T1a below and done before
    going on; with T1a, `interpreter_reentry_crosses_threads` moves one interpreter through three threads: `["0001", "0002", "0003"]`.
    Both pass with and without `--features sql`.
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (test module only, unless the test fails).
  - Do: unit test `interpreter_reentry_keeps_working_storage` — one `Interpreter`, a counter program, `run()` twice: answers 1 then 2; with
    the `sql` feature, `sql_end` has run at the end of each `run` (no transaction carried across). A compile-time assertion that `Interpreter`
    is `Send` (`fn assert_send<T: Send>() {}`), which `spawn_blocking` needs.
  - **If either fails, stop here:** the fix is a separate runtime task with interpreter/binary parity (`rcrun run-form`, embedded child forms,
    the compiled binary, the backend — `.claude/skills/interpreter-binary-parity`), recorded in this list before T5 starts. No workaround
    inside `cobolt-server`.
  - Verify: `cargo test -p cobolt-runtime interpreter_reentry --no-fail-fast` and `cargo test -p cobolt-runtime --features sql interpreter_reentry`;
    the test prints the two answers.

- [x] **T1a — `IndexedStore` is `Send`, so `Interpreter` is** (runtime; the separate task T1 calls for)
  - Files: `crates/cobolt-runtime/src/indexed.rs` (`pub trait IndexedStore: Send`). Every implementor — `IndexedFile` (`indexed.rs`),
    `DiskIndexedFile` (`indexed_disk.rs`), `RedbIndexedFile` (`indexed_redb.rs`) — already was; nothing else in `Interpreter` blocked `Send`.
  - Parity: a trait bound, no behaviour — `rcrun run-form`, embedded child forms, the compiled binary and the backend run the same code.
  - Verify: `cargo test -p cobolt-runtime --lib interpreter_reentry` (2 pass, with and without `--features sql`);
    `cargo check -p cobolt-runtime --no-default-features` clean; the full runtime suite in T14.

## One list of exports, one property (plan C2, C5, §3; D5)

- [x] **T2 — The scaffold writes `backend/exports.toml` and a development `backend/backend.toml`** (R1, R22; C5)
  - Files: `crates/cobolt-project-tools/src/create.rs` (beside `BACKEND_STARTER` and `starter_backend`; the `[backend]` table write `:531`).
  - Do: a new Web or Mobile project gets `backend/exports.toml` exporting `HEALTH` with `public = true`, and a `backend/backend.toml` that
    listens on loopback, has no secret and no `unsafe_*` setting. Both carry the generated-file comment the other scaffold files carry (they
    are scaffold, written once — the developer's thereafter; never rewritten over an existing file). The `[backend] exports` line in
    `cobolt.toml` is no longer written (D5); `state` is not written either (absent = `per-call`, T3). The WASM-009 authentication programs are
    **not** added here (plan §2.1: M5 adds them when they exist).
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` — a new test creates a Web project in a temp folder and asserts both files,
    then runs `rcrun backend check <dir>` on it (exit 0, prints the count of exports); an existing `exports.toml` is left byte-identical.
  - *Result (2026-10-23):* `create.rs` writes `BACKEND_EXPORTS` (`HEALTH`, `public = true`) and `BACKEND_CONFIG` (`[server] listen =
    "127.0.0.1:8443"` only), neither in `[files] backend` (the Check reads that list as COBOL); `[backend] exports` is no longer written.
    `kinds::a_new_backend_has_its_exports_and_a_development_deployment` (web, android-phone; an existing `exports.toml` left byte for
    byte). The `rcrun` half lives where `rcrun` is built: `backend_cli::a_new_web_projects_backend_checks_clean_and_answers_its_health_probe`
    — `contract 1.0.0: 1 exported program(s)`, `backend run` up in 56 ms, `/health` 200. cobolt-project-tools 156 pass / 0 fail / 1 ignored;
    `backend_cli` 5 pass.

- [x] **T3 — `[backend] state`, and `exports` retired as a source** (R18, Q2 — *operator*; C2, C5)
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`BackendSettings`), `crates/cobolt-compiler/src/lib.rs` (`ManifestView::backend_exports`
    re-pointed at `backend/exports.toml` through the `cobolt-server` loader the compiler already links), `crates/cobolt-server/src/bundle.rs`
    (`Bundle` gains `state`), `crates/cobolt-compiler/src/backend_build.rs` (copies `state` into the bundle), `crates/cobolt-cli/src/main.rs`
    (`cmd_backend` `run` reads `[backend] state` from the project's `cobolt.toml`).
  - Do: `state: String` with `#[serde(default)]` → `"per-call"`; accepted values `per-call` | `per-connection` (**O2:** the plan's spelling;
    the visible words are Tr keys, T9). Any other value is a Check/Build error naming the two values. `exports` still deserialises (old
    manifests load) but nothing reads it as the list. An absent `[backend]` table or `state` = `per-call`, so every existing project is unchanged.
  - Verify: `cargo test -p cobolt-compiler --no-fail-fast` (a round-trip test: absent → `per-call`; `per-connection` survives a save; `"session"`
    is refused with the message) and `cargo test -p cobolt-server --no-fail-fast` (bundle round-trip carries `state`).
  - *Result (2026-10-23):* `cobolt_server::backend::StateMode` (loader) parses the two spellings; `BackendSettings::state` (not written
    when default), `ManifestView::{backend_exports (from exports.toml via `backend::exported_names`), backend_state, backend_state_mode}`,
    `backend_build::backend_state` (Build refuses a bad word; the bundle carries the mode), `rcrun backend` checks it for every verb and
    passes it to `DevOptions::state`, the project-tools Check reports it. `project_kind::tests::backend_state_round_trips_and_refuses_an_unknown_word`,
    `bundle::tests::a_bundle_round_trip_carries_the_state` pass; cobolt-server 63 pass; cobolt-compiler: every binary green except 4 lib tests
    that fail on the SDK crate set (`cobolt-decimal`, `cobolt-numedit` not shipped — `sdk_crate_set_is_closed_under_path_dependencies` and
    three `resolve_main_tests`), untouched by this change.

## The server (plan §2.2–§2.6; D3, D4)

- [x] **T4 — `unsafe_plain_ws` is removed** (R5, Q7 — *operator*; D4; AC4)
  - Files: `crates/cobolt-server/src/config.rs` (the field `:233`, its warning `:416`, the `unsafe_set` list), `crates/cobolt-server/tests/config_rules.rs`.
  - Do: plain `ws` is served only on a loopback address, in every start mode (that is the reverse-proxy arrangement of Q7). A `backend.toml`
    still naming `unsafe_plain_ws` fails to start with a specific message ("`unsafe_plain_ws` was removed: plain ws:// is served on loopback only;
    use TLS …"), not the generic unknown-field one. *Reading recorded (plan D4), not decided here:* R5's wording should say "loopback `ws` also in
    a release start, behind a reverse proxy" at the next `/clarify`.
  - Verify: `cargo test -p cobolt-server --test config_rules --no-fail-fast` — `plain_ws_is_never_served_on_a_non_loopback_address` replaces the
    `unsafe_plain_ws` case of `plain_ws_is_for_loopback_only_and_tls_is_all_or_nothing`, and reports each address tried and its outcome;
    `cargo test -p cobolt-server --test loopback a_foreign_origin` still passes.
  - *Result (2026-10-23):* field, warning and `unsafe_set` entry gone from `config.rs`; `ServerConfig::from_toml` answers a file naming it
    with "`unsafe_plain_ws` was removed: plain ws:// is served on a loopback address only (WASM-003 R5); use TLS …". The new test tries seven
    addresses in both modes: `0.0.0.0`, `192.168.1.20`, `[::]`, `10.0.0.5` refused in Release and Development; `127.0.0.1`, `[::1]`,
    `127.0.0.2` served. `config_rules` 11 pass (`tls_is_all_or_nothing` keeps the TLS half of the old test); `a_foreign_origin…` passes
    (its `unsafe_plain_ws` assertion now expects the removal message). *Not touched:* the frontend's own `unsafe_plain_ws`
    (`cobolt-web-player` `net.rs`/`api.rs`, `cobolt-web-bundle` `lib.rs`) — a client setting, outside this task's files.

- [x] **T5 — `per-connection` sessions** (R18, R19, Q2 — *operator*, Q4; D3) — *requires T1 green and T3*
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
  - *Result (2026-10-23):* `exec::Session` (one `Unit` per exported program, one shared `ExternalStore` through the new runtime constructor
    `Interpreter::new_with_channels_and_store`; `admit` binds the first subject); `exec::execute` is the same body as before, now `run_call`
    on a fresh unit. `Backend::state` (set by `Bundle::into_backend` and `DevServer::start`), `Conn::state` in `serve.rs`, claims installed at
    every call, `LogRecord::state`; Build notes an `independent` export in `per-connection` mode. A call cut off, cancelled or faulted
    drops that program's run unit (its next call starts fresh); a refused payload or a `WS-FAIL` keeps it.
    `per_connection_state_is_kept_and_never_shared`: alice `[1, 2, 3]`, bob `[1, 2, 3]` interleaved; bob's token on alice's connection →
    `authorisation 3000`; alice again → 4; reconnected → 1; `per-call` `[1, 1, 1]`; every call record names its mode.
    `a_file_held_open_across_calls_makes_others_wait_or_93`: the other connection waited 404 ms (limit 400) → 93; after the holder's `CLOSE`
    call → 00; after the holder disconnected → 00 in 14 ms. cobolt-server 66 pass (`loopback` 13), cobolt-wire 23 pass.
  - *Deviations, stated:* (1) **Serialised by the ordering ticket, not by a per-connection permit of 1** — `on_call` takes the permit with
    `try_acquire`, so a permit of 1 would *refuse* a second pipelined call (7002) instead of queueing it, against R6 ("requests may be in
    flight together"); `independent` is ignored instead, so the calls of a stateful connection run in turn. (2) **No `session: "new"` in
    `ServerHello`** — WASM-014 defines `hello.session` as a *client*-made id that a reconnect carries to *continue* a session for
    idempotency (WASM-014 plan §2.4 D-Session); it defines no server member saying "new". Adding one would decide WASM-014's wire, so it
    is left open (see *Open items carried*); a reconnect starts fresh state whatever the client sends. (3) Stream subscriptions keep
    running per call in either mode.

## The Leptos front (plan C1, C3; D1, D2, D6; §5 R2, R5) — O1 working assumption

**O1 is open** (WASM-003 R2 names `cargo-leptos`; WASM-017 R17 keeps `cargo-leptos` in the product's build only). **Working assumption, not
decided:** the backend is built on the Leptos framework inside `cobolt-server` with plain `cargo build`; `cargo-leptos` is not needed on the
developer's machine. If the operator rules otherwise, T7 gains a tool check with a missing-tool message (Tr ×6) and WASM-017 R17 needs an amendment.

- [x] **T6 — The Leptos server crates enter behind `server`, guarded first** (R2; plan §5 R2)
  - *Result (2026-10-23):* probed first in a throwaway project outside the repository: `leptos` 0.8.22 (`ssr`), `leptos_axum` 0.8.10,
    `server_fn` 0.8.13, `axum` 0.8.9, `hyper` 1.12.0, `hyper-util` 0.1.21 — 210 crates, none of `ring`/`aws-lc*`/`rustls*`/`openssl*`; the
    highest `rust-version` in the closure is 1.89 (`uuid`), Leptos's own 1.88. Workspace dependencies added, named only by `server`.
    `dependency_guards` 4 pass: server 406 crates with the 7 front crates and no headless/barred name; loader 47 and client 227 with
    neither; the compiler links no `leptos`/`leptos_axum`/`server_fn`. The old guard's `hyper` (the runtime's maps bridge) is now
    `reqwest` + `aws-config`, since `hyper` is the front's. `cargo check -p cobolt-server --no-default-features --features loader` clean.
    `Cargo.lock` +949 lines, no version of an existing crate changed. Note: `rcrun` links `cobolt-server` with its defaults, so it now
    carries the front too (it serves `rcrun backend run`); the IDE links no `cobolt-server`.
  - Files: root `Cargo.toml` (`[workspace.dependencies]`: `leptos`, `leptos_axum`, `axum`, `hyper`, `hyper-util`, `default-features = false`,
    server side `ssr` only), `crates/cobolt-server/Cargo.toml` (`server` feature names them; `loader` and `client` do not),
    `crates/cobolt-server/tests/dependency_guards.rs`.
  - Do: add the dependencies and nothing that uses them yet. Extend the guards: no `ring`, `aws-lc-*`, `rustls*`, `openssl*` in the `server`
    closure; no `leptos*`, `axum`, `hyper*` in the `loader` closure (`the_compiler_reads_and_packs_a_backend_through_the_loader_alone`) nor in
    the `client` closure. Check each added crate's `rust-version` ≤ 1.92. **If a barred crate or a newer MSRV is unavoidable, stop and report;
    the guard is not weakened.**
  - Verify: `cargo test -p cobolt-server --test dependency_guards --no-fail-fast` (each guard prints the closure size and the barred names it
    found: none); `cargo check -p cobolt-server --no-default-features --features loader`.

- [x] **T7 — The HTTP front moves onto the Leptos/axum router; the session code does not change** (R2, R5, R8, R14, R16, Q3 — *operator*; D1, D6)
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
  - *Result (2026-10-23):* `serve.rs`: `connection` keeps the connection limit and the TLS handshake timeout and hands the stream to
    `serve_http` — hyper's `http1` (`header_read_timeout` = handshake timeout, `max_buf_size` = `http::MAX_HEAD`, title-case headers) with
    upgrades, serving `front_router`: an `axum::Router` with `/health`, `/dl/{token}`, `/` and `/ws`, a 404 fallback, any other method 404,
    and the security headers plus `Connection: close` as a response layer (not on the 101). `upgrade` checks Upgrade/Connection, the
    Origin (`origin_allowed`, now taking the header value), `Sec-WebSocket-Version: 13` and the key, answers 101 with
    `derive_accept_key`, and runs `session` unchanged on `WebSocketStream::from_raw_socket(TokioIo<Upgraded>)` with the same
    `WebSocketConfig`; the connection lasts as long as its session. `download` streams with `Content-Length` and `Range` as before.
    `health_loop` serves a `/health`-only router (the plain listener must not offer the upgrade). `http.rs`: `read_head`, `Head`,
    `HeadError`, `Replay` and `respond` gone; `SECURITY_HEADERS` is a list of pairs. `httparse` dropped. `on_hello`/`on_text`/`on_call`
    and `jwt.rs` untouched. cobolt-server 68 pass: lib 10, `client_contract` 13, `config_rules` 11, `dependency_guards` 4, `loopback` 15,
    `token_rules` 15. `routes_are_closed`: `GET /health` 200; `POST /health`, `/dl/<bad>`, `/dl/../etc/passwd`, `GET /ws` and `GET /`
    without upgrade, `/admin`, `/api/anything`, `/ws/extra` (upgrade), `/health/` → 404; upgrade `/` and `/ws` → 101.
    `a_large_or_slow_request_head_is_refused`: a 20 KiB head → 431; an unfinished head closed after 1502 ms (limit 1500).
  - *Stated plainly — what of Leptos runs:* the router is `axum`'s, the server `hyper`'s; **no `leptos`/`leptos_axum` API is called**.
    A closed table of four platform routes has no page, no hydration and no server function, and `leptos_axum`'s entry points
    (`LeptosRoutes::leptos_routes`, `render_app_to_stream`, `handle_server_fns`, `file_and_error_handler`) all serve one of those. The
    crates are linked (T6) and are what WASM-009's identity routes would mount through, on this router. Whether that satisfies R2's
    "built on Leptos" is the operator's reading, joined to **O1**; nothing here is hard to change either way.

- [x] **T8 — The built binary on the new front** (R2, R3, R4, Q6 — *operator*; AC1)
  - Files: `crates/cobolt-compiler/tests/backend_build_wasm003.rs` (assertions added), `crates/cobolt-compiler/src/backend_build.rs` only if
    the bundle/`state` change needs it.
  - Do: build a backend with one exported and one non-exported program; the staging `Cargo.toml` names `cobolt-server` and no `leptos*`/`axum`
    crate; the binary answers `/health`, `--describe` prints `interface.json`, the non-exported program is `not-found 5000`.
  - Verify: `cargo test -p cobolt-compiler --test backend_build_wasm003 --no-fail-fast`; the test prints the binary size and the cold build time,
    recorded in T12 against 53.1 MB / 29 s (plan §5 R5). `cargo test -p cobolt-cli --test backend_cli --no-fail-fast` (3) still green.
  - *Result (2026-10-23):* `backend_build_wasm003` 3 pass. The e2e build adds a non-exported `SECRET`; the generated `Cargo.toml`
    names `cobolt-server` and none of `leptos`/`axum`/`hyper`; `--describe` = `interface.json`; `/health` 200, `/other` 404 (the
    health listener); over the built binary's WebSocket (a minimal client in the test), `PING` → `pong`. **40 s** cold debug build,
    **57.5 MB** (was 29 s / 53.1 MB on the hand-rolled front). `backend_cli` 5 pass (the 4 it had — not 3 as written above — and T2's).
  - *Deviation, stated:* the built binary runs as **release**, which refuses the development token, and this test has no identity
    provider; so the not-exported check there is that `SECRET` (3 spellings) answers **exactly** like `NO-SUCH-PROGRAM` — both
    `authentication 2000`, the "no token" an unauthenticated caller gets for any non-public name, so names cannot be enumerated (plan §1).
    The `not-found 5000` with a valid token is proven on the same server code by
    `loopback::an_exported_program_answers_and_nothing_else_does` (nine spellings, T7's run).

## The IDE (plan C5, §3; R20; AC9)

- [x] **T9 — The Backend settings section edits `backend/exports.toml` and `state`; old projects migrate once** (R1, R18, R20; C2, C5; O2)
  - Files: `crates/cobolt-ide/src/panels/web_settings.rs` (the exports field `:357` reads and writes `backend/exports.toml` — names added
    as `[[export]] program = …`, an existing entry's `roles`/`public`/`idempotent`/… preserved, never dropped; a `state` choice of two values),
    the project-open path in `crates/cobolt-ide/src/app.rs` (the migration), `crates/cobolt-ide/src/i18n.rs`.
  - Do: on open, `backend/exports.toml` absent and `[backend] exports` non-empty → write `exports.toml` from the names (no roles, not public)
    and say so in the Output panel; `cobolt.toml` untouched (user code is sacred). New `Tr` keys in **all six** languages: the `state` label,
    its two values (O2: the visible words are the operator's to change), the migration notice; `pk_set_exports_hint` re-worded to name the file.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — `opening_a_project_with_old_exports_writes_exports_toml_and_leaves_cobolt_toml`
    (asserts the new file and `cobolt.toml` byte-identical), an edit round-trip that keeps an entry's `roles`, and the i18n test (no empty
    translation in the six tables). Window check per the egui-resize memory: the section at its default size in six languages.
  - *Result (2026-10-23):* `web_settings.rs`: `exports_path`, `read_exports_text`, `write_exports` (`toml_edit`: a kept entry keeps its
    keys and comments, a new name is bare, a removed one goes, unchanged → no write), `migrate_exports`, the `state` row (`STATE_PER_CALL`
    / `STATE_PER_CONNECTION`, words from `Tr`); `apply_kind` writes `state` (only when not the default or a `[backend]` exists) and no
    longer writes `exports`. `SettingsDraft::backend_state`; `SettingsForm::load_backend_exports` and `exports_edited` — the file is
    written on Save **only if the box was edited**, so a file changed by hand is never overwritten by an untouched box. `app.rs`:
    `open_project_at` migrates once and reports it, then fills the box from the file; `save_settings_form` writes the file.
    `BackendSettings::exports` is now skipped when empty, so a manifest saved by the IDE gains no `exports = []`. Six `Tr` keys ×6
    languages (`pk_set_state`, `_per_call`, `_per_connection`, `_hint`, `pk_set_exports_migrated`, `pk_set_exports_write_failed`),
    `pk_set_exports_hint` re-worded to name the file. `toml_edit` 0.22 added to the IDE (already in the lock via cobolt-project-tools).
    Tests: `opening_a_project_with_old_exports_writes_exports_toml_and_leaves_cobolt_toml` (HEALTH, ORDERS → 91-byte file; project file
    161 bytes unchanged; second open writes nothing), `an_exports_edit_keeps_each_entrys_roles` (ORDERS keeps roles/idempotent and its
    comment; REFUND added bare; HEALTH removed; same list again → no write), `the_state_is_written_only_when_it_is_not_the_default`,
    the round-trip test updated; web_settings 11 pass; i18n 19 pass; the section drawn in 6 languages × 3 kinds (72 frames) within its width.
    The test calls `migrate_exports` directly — `open_project_at` is the one line that calls it; no test drives the IDE's window.

- [ ] **T10 — Start and stop the backend from the IDE, its log in the Output panel** (R20; AC9 IDE half) — *design not in the plan*
  - **Decided by the operator (2026-10-23): a child process.** The IDE runs `rcrun backend run <project>` (the `sibling_rcrun` pattern),
    streams its stdout/stderr into the Output panel and kills it on Stop and on IDE exit; the IDE links no `cobolt-server`, no Leptos and no
    socket/TLS/token crate (extend the T6 guard to assert it).
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

- [x] **T11 — System KB: *Calling the backend* rewritten; `chunked.data` regenerated** (R12, R18, R19; Q2)
  - *Result (2026-10-23, done ahead of T10 because the KB rule ties it to T5's behaviour change):* in *Calling the backend*
    (`crates/cobolt-compiler/src/lib.rs`): `exports.toml` is the one list and the one-time copy from `[backend] exports`; the `state`
    property (`per-call` default; `per-connection` — kept per connection, calls in turn, one user per connection, a cut-off/cancel/fault
    restarts that program, nothing kept after the connection); the file lock held across a `per-connection` connection's calls; plain
    `ws://` on loopback only in every mode, the scaffold's two files, and `/health` with 404 for the rest. `build_chunked_kb`: 3553
    records from 8 documents, `chunked.data` 12 730 368 → 12 738 560 bytes; `prebuilt_chunked_kb_matches_the_published_documentation` pass.
  - Files: `crates/cobolt-compiler/src/lib.rs` (the section at `:5130`; the sentence at `:5154` "Every call runs in a fresh interpreter"
    becomes the `state` property: `per-call` default, `per-connection` keeps `WORKING-STORAGE` per connection, serialised, never resumed
    across connections, a file left open holds its lock across that connection's calls; `backend/exports.toml` is the one list),
    `assets/knowledge/chunked.data` (regenerated).
  - Do: `cargo run -p cobolt-ide --example build_chunked_kb` in the same change. `WS-CLAIM` is already in `BUILTINS`; no new built-in.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`; `git diff --stat` shows
    `chunked.data` changed.

- [ ] **T10a — The front runs on Leptos for real** (R2; O1 **decided 2026-10-23**: "use Leptos de fato")
  - The operator ruled that a front built on hyper + axum with Leptos linked but never called does not satisfy R2. The HTTP front is served
    through Leptos: the router is `leptos_axum`'s, and the platform routes (health, hello/token exchange, the WebSocket upgrade, the static
    answers) go through Leptos server functions or Leptos-registered handlers rather than a bare axum table. The WebSocket session code, the
    431 limit, the closed route set and every guard of T6–T8 stay as they are. Still plain `cargo build`: `cargo-leptos` is not needed on the
    developer's machine, so WASM-017 R17 holds.
  - Verify: the T7/T8 tests pass unchanged on the Leptos front; a test proves each platform route is answered by the Leptos layer (e.g. a
    route registered only through Leptos answers, a route absent from Leptos's table gets 404); the dependency guard still holds; T13 then
    re-runs its evidence on this front.

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

### State of this run (2026-10-23)

T1–T9 and T11 done; **stopped at T10** (an operator decision); T12 and T13 not run. T14's version and CHANGELOG parts were done because the
run required them: `2.0.37` → `2.0.38`, one entry above 2.0.37. T14 stays unchecked. Sweep with `--no-fail-fast`, every `test result` line read:

| Crate | Passed | Failed | Ignored |
|---|---|---|---|
| cobolt-runtime | 1554 | 0 | 29 |
| cobolt-wire | 23 | 0 | 0 |
| cobolt-server | 68 | 0 | 0 |
| cobolt-compiler | 207 | 4 | 5 |
| cobolt-cli | 62 | 0 | 5 |
| cobolt-project-tools | 156 | 0 | 1 |
| cobolt-ide (`--bin cobolt-ide`) | 1875 | 5 | 4 |

None of the 9 failures touches this change:
- **cobolt-compiler (4):** the SDK crate set does not ship `cobolt-decimal`/`cobolt-numedit` (`sdk_crate_set_is_closed_under_path_dependencies` and three `resolve_main_tests`).
- **cobolt-ide (5):**
  - `every_document_ships_in_every_language`: docs translations, removed under the 2.x.x rule;
  - `every_completion_description_exists_in_six_languages`: 12 descriptions are English only, none of them touched here;
  - `claude_validates_the_written_bundle`: the external `claude` plugin validator;
  - `compatible_and_reserved_collisions_still_refuse` and `live_resolver_verdicts`: live crates.io, where egui 0.37 is now published.

`cargo fmt --check` fails across 887 files of the workspace, most of them not touched here: the codebase is written in wide lines. Nothing was reformatted. The credential scan of the diff found nothing.

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
- **T5, the `hello` member** — the plan wants the client told by `hello` that a reconnection starts with fresh state; WASM-014's
  `hello.session` means the opposite direction and purpose (a client id that continues an idempotency session). Which member, if any,
  says "state is new" is WASM-014's to define; nothing was added to `proto.rs`.

## Done criteria
All acceptance criteria in spec.md are checked with the evidence of T13, the tests of every touched crate pass, the System KB and
`chunked.data` agree, the six i18n tables have the new keys, and the version and CHANGELOG are updated once. Commits and pushes are the
pipeline's, not this list's.
