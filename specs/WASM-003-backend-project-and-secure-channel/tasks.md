<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The Backend project and the secure frontend–backend channel (WASM-003)

- **Status:** revised **2026-10-31** against the plan of 2026-10-31 (plan §0.1 C7–C14). The 2026-10-20 work (M3) and **T1–T14
  (2026-10-23, 2.0.39)** are done and kept below unchanged, with their evidence. **Open:** T15a (R25, R26; AC12; plan §2.8) and T15b (R24;
  AC11; plan §2.7), expanded below into steps, each with its own Finalize (plan §8: "T15a and T15b each have one").
- **Plan:** ./plan.md   **Date:** 2026-10-22; revised 2026-10-31

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

- [x] **T10 — Start and stop the backend from the IDE, its log in the Output panel** (R20; AC9 IDE half) — *design not in the plan*
  - *Result (2026-10-23):* `crates/cobolt-ide/src/backend_run.rs` — `BackendRun::spawn(rcrun, project_dir)` runs `rcrun backend run
    <project>` with stdout/stderr read on two threads; `poll` yields each line, the address (`Listening`) and an exit that came by itself;
    `stop` kills and waits; `Drop` stops it, so the IDE's exit (and opening another project, `open_project_at`) leaves no backend behind.
    `app.rs`: Run menu → **Start Backend** / **Stop Backend**, shown when the open Web/Mobile project has `backend/exports.toml`
    (`backend_project_dir`), `start_backend`/`stop_backend`, `poll_backend` once a frame into the Output panel. `rcrun backend run` now
    prints each log record to stderr **as it is written** (`DevOptions::echo`, `dev.rs` `EchoSink`) instead of all of them at Ctrl-C, so the
    panel is live (the terminal sees the same lines, earlier). Five `Tr` keys ×6 (`backend_start`, `backend_stop`, `backend_running`,
    `backend_stopped`, `backend_start_failed`). Guard: `dependency_guards::the_ide_runs_the_backend_as_a_child_and_links_none_of_the_server`
    — IDE closure 705 crates, `cobolt-server` features `{"loader"}` only (through the compiler), none of `leptos`/`leptos_axum`/`server_fn`/
    `axum`/`jsonwebtoken`/`tokio-tungstenite`. (The IDE does link `hyper` and `native-tls` through its HTTP and agents crates, as before.)
    Tests: `backend_run::tests::the_ide_starts_the_backend_reads_its_log_and_stops_it` — a new Web project, listening line after 2205 ms
    (`ws://127.0.0.1:65328/ws`), `GET /health` 200, 6 lines captured live incl. the `start` record, stopped in 0 ms, port free at once;
    `a_folder_without_a_backend_exits_and_says_why` (the `exports.toml` message, `Exited(Some(1))`). The test needs `target/debug/rcrun`
    built (`cargo build -p cobolt-cli --bin rcrun`; `--bin cobolt-ide` does not build it) and says so when it is missing. i18n 19 pass;
    `backend_cli` 5 pass; `dependency_guards` 5 pass. Manual check (operator): Run → Start Backend on a Web project, `curl
    http://127.0.0.1:8443/health` (the scaffold's `backend.toml` listens there, plain on loopback), Run → Stop Backend.
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

- [x] **T10a — The front runs on Leptos for real** (R2; O1 **decided 2026-10-23**: "use Leptos de fato")
  - *Result (2026-10-23):* new `crates/cobolt-server/src/front.rs`. The closed route table is a **Leptos route table** —
    `PlatformRoutes`, a `leptos_router` `<Router><Routes>` with `/`, `/ws`, `/dl/:token` — read once by
    `leptos_axum::generate_route_list` (`front::route_list`) and mounted in `serve.rs` `front_router` with
    `LeptosRoutes::leptos_routes_with_handler`: the upgrade listings → `upgrade`, the download listing → `download`, both unchanged.
    `GET /health` is a **Leptos server function** (`#[server(prefix = "", endpoint = "health", input = GetUrl)] front::health`, JSON
    output) run by `leptos_axum::handle_server_fns_with_context` with the server's `Arc<Shared>` provided as context; `serve::health_value`
    is its body (the plain `health_listen` listener runs the same server function). Anything else: the 404 fallback, and
    `method_not_allowed_fallback` keeps a wrong method on a Leptos route a 404 as before. `LeptosOptions: FromRef<ConnCtx>`
    (`front::options`, defaults named `backend`) is what `leptos_axum`'s router extension asks of the state. No page, no hydration, no
    client crate; still plain `cargo build`, no `cargo-leptos` (WASM-017 R17 holds). `session`, `on_hello`/`on_text`/`on_call`, `jwt.rs`,
    the 431 cap and the handshake timeout untouched.
  - Dependencies: `leptos_router` 0.8.17 (`ssr`) named directly; `leptos_axum` now keeps its **default features** — they are its tokio
    executor (without them `generate_route_list` prints a "default-features = false" warning on every start) and `server_fn/axum`, the
    server-function service. *Cost, stated:* `server_fn/axum` turns on `axum/ws`, so the server closure gains a second, TLS-free
    `tokio-tungstenite`/`tungstenite` **0.29** (the session keeps 0.28), plus `tower-http` 0.6.11, `http-range-header`,
    `serde_path_to_error`: 406 → 411 crates; none barred; highest `rust-version` in the closure 1.90 (`redb`). `Cargo.lock` +70/−4 (the
    4 are dependency lines that now name `tokio-tungstenite`/`tungstenite` with their version; no existing crate changed version).
  - Tests: `front::tests::the_platform_routes_are_leptos_s_table_and_the_probe_a_server_function` — table `["/", "/dl/{token}", "/ws"]`,
    `/admin` absent, server functions `[("/health", "GET")]`. Unchanged and green on this front: cobolt-server **70** pass (lib 11,
    `client_contract` 13, `config_rules` 11, `dependency_guards` 5, `loopback` 15, `token_rules` 15); `routes_are_closed`: `GET /health`
    200; `POST /health`, `/dl/<bad>`, `/dl/../etc/passwd`, `GET /ws` and `GET /` without upgrade, `/admin`, `/api/anything`, `/ws/extra`,
    `/health/` → 404; upgrade `/` and `/ws` → 101; `a_large_or_slow_request_head_is_refused`: 20 KiB head → 431, unfinished head closed
    after 1503 ms (limit 1500). `backend_build_wasm003` 3 pass: **71 s** cold debug build, **67.2 MB** (T8 on the axum front: 40 s /
    57.5 MB; hand-rolled: 29 s / 53.1 MB — the Leptos code is now used, so it is no longer dropped by the linker; the build time was
    measured with other builds running); `backend_cli` 5 pass.
  - The operator ruled that a front built on hyper + axum with Leptos linked but never called does not satisfy R2. The HTTP front is served
    through Leptos: the router is `leptos_axum`'s, and the platform routes (health, hello/token exchange, the WebSocket upgrade, the static
    answers) go through Leptos server functions or Leptos-registered handlers rather than a bare axum table. The WebSocket session code, the
    431 limit, the closed route set and every guard of T6–T8 stay as they are. Still plain `cargo build`: `cargo-leptos` is not needed on the
    developer's machine, so WASM-017 R17 holds.
  - Verify: the T7/T8 tests pass unchanged on the Leptos front; a test proves each platform route is answered by the Leptos layer (e.g. a
    route registered only through Leptos answers, a route absent from Leptos's table gets 404); the dependency guard still holds; T13 then
    re-runs its evidence on this front.

- [x] **T12 — AC10 and R23: a secret stops the frontend build, the same value in `backend.toml` does not; `connect-src` printed**
  - *Result (2026-10-23):* the test is `crates/cobolt-project-tools/tests/backend_secret_not_scanned.rs`, **not** under
    `cobolt-web-bundle/tests` as written below — the bundle crate takes already-assembled inputs, so "nothing under `backend/` reaches the
    bundle" can only be shown where a project folder becomes a bundle (`web::generator::GeneratorBackend`, beside WASM-002 T17's
    `secret_in_a_wcfrm_property_stops_the_build_naming_file_and_pattern`). One Web project with a Backend: the planted value (AWS's
    documented example key, built from fragments) in a `.wcfrm` caption → stopped, "`forms/main-form.8b03d742.cfrm: line 10: secret
    (cloud access key id); the value is not shown`", nothing written; the same value only as `backend/backend.toml` `[tls]
    pkcs12_password` → built, 25 files / 7 429 584 bytes, no path from `backend/`, no file holding the value; `connect-src 'self'
    wss://api.example.com`, and the page carries it.
  - **Two code changes the test needed, stated:** (1) *a bug:* `generator.rs` turned `[web] backend_url` into an origin with
    `cobolt_web_bundle::origin_of`, which answers `http(s)` only, so a `wss://` backend — the only kind `WebSettings::backend_url` documents
    and the bundle accepts — was silently left out of `connect-src` (the first run printed `connect-src 'self'`). New
    `generator::backend_origin` keeps the `ws`/`wss` scheme and passes any other value whole, for the bundle to refuse by name (as an
    `https://` value already was); `generator::tests::the_backend_origin_keeps_the_websocket_scheme`. (2) *R23's "print":* no build path
    reported the `connect-src` (the bundle's own `BuildReport.csp_meta` never reached the project report); `web::BuildReport` gains
    `connect_src`, filled by the generator and answered in `to_json` (what `rcrun`/the agent tools print). The IDE's Output panel does
    not show it yet — that needs a `Tr` label; left open, not done here.
  (R17, R23; AC10) — *the scan itself is WASM-002's (T2 there, done)*
  - Files: `crates/cobolt-web-bundle/tests/backend_secret_not_scanned.rs` (new, test only).
  - Do: one project fixture: a planted test secret in a form stops the bundle (naming file and pattern, never the value); the same value only
    in `backend/backend.toml` leaves the bundle building, and no file under `backend/` is in the bundle; the build report names the
    `connect-src` (`csp.rs`) for the configured backend address.
  - Verify: `cargo test -p cobolt-web-bundle --test backend_secret_not_scanned --no-fail-fast` — prints the two outcomes and the `connect-src`.

## Evidence and finalize

- [x] **T13 — Evidence re-run on the Leptos front, recorded here** (AC1–AC10; plan §5 R5, R7) — see *Evidence (re-run, 2026-10-23)* below
  - Do: `df -h .` first (a test sweep links many binaries). Re-run every suite the plan §6 names and append an **Evidence (re-run)** table to
    this file: one row per acceptance criterion, the test that proves it, the printed numbers; AC9 and AC10 rows from T10 and T12. State that
    Windows and Linux were not run, if they were not, and that no real identity provider was contacted. Cross-spec risk R1 (the player's
    `{"type":…}` envelope vs `proto.rs` `{"t":…}` with `hello`, `net.rs:264` vs `parse_client`) is recorded, not fixed — WASM-014/WASM-016 own it.

- [x] **T14 — Finalize** (all) — `2.0.38` → **`2.0.39`** (once, this run), one CHANGELOG entry above 2.0.38; the sweep below
  - Files: `crates/cobolt-ide/src/version.rs` (bump `z` **once**: `2.0.29` → the next free `2.0.z` at the time), `CHANGELOG.md` (one entry
    **above** the previous heading, `## [PowerRustCOBOL 2.0.z] — <date>`, marked *Written on features-2x*, listing T1–T13). **No documentation
    task:** `README.md` and `docs/` are not touched on 2.x.x.
  - Verify: the tests of every crate touched, with `--no-fail-fast`, every `test result` line read:
    `cargo test -p cobolt-runtime --no-fail-fast`, `cargo test -p cobolt-wire --no-fail-fast`, `cargo test -p cobolt-server --no-fail-fast`,
    `cargo test -p cobolt-compiler --no-fail-fast`, `cargo test -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-project-tools --no-fail-fast`,
    `cargo test -p cobolt-web-bundle --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`; `cargo fmt --check`;
    the credential scan of the staged diff (no `AKIA`/`ASIA`/secret) before the pipeline commits.

### State of the previous run (2026-10-23, 2.0.38 — superseded by the evidence above)

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

### Evidence (re-run, 2026-10-23, on the Leptos front) — T13

macOS (Darwin 24.5, aarch64), debug builds, real sockets on loopback; the numbers are what the tests printed. **Windows and Linux were
not run. No real identity provider was contacted** (a local test issuer supplies the keys).

| AC | Proved by | Result |
|---|---|---|
| AC1 exported vs non-exported, one executable | `loopback::an_exported_program_answers_and_nothing_else_does`; `backend_build_wasm003` (3) | pass; nine spellings `not-found 5000`; built binary 71 s / 67.2 MB, `/health` 200, `PING` → `pong`, `SECRET` answers like `NO-SUCH-PROGRAM` |
| AC2 token refusals | `loopback::tokens_are_accepted_…`, `token_rules` (15) | pass |
| AC3 role before start, subject readable | `loopback::a_role_is_checked_before_the_program_starts`, `backend_cli` (`WHOAMI`) | pass |
| AC4 foreign Origin, non-loopback `ws` | `loopback::a_foreign_origin_…`, `config_rules::plain_ws_is_never_served_on_a_non_loopback_address` | pass |
| AC5 payload validation | `loopback::a_payload_that_does_not_fit_…` | pass |
| AC6 size, time, rate | `loopback::size_time_and_rate_limits_…`, `a_large_or_slow_request_head_is_refused` | pass; timeout at 702.7 ms (limit 700); burst 10 → 4 answered, 6 × 7001; 8 kB frame → 7000; 20 KiB head → 431; unfinished head closed at 1503 ms (limit 1500) |
| AC7 concurrent clients isolated | `loopback::concurrent_callers_…`, `per_connection_state_is_kept_and_never_shared` | pass |
| AC8 no token/password in the log | `loopback::the_log_has_neither_the_token_nor_a_field_…` | pass |
| AC9 IDE start/stop, binary identical | `backend_run::tests::the_ide_starts_the_backend_reads_its_log_and_stops_it`, `a_folder_without_a_backend_exits_and_says_why`; `backend_cli` (5) | pass; address after 2205 ms, `/health` 200, live `start` record, stopped in 0 ms, port free. The IDE runs `rcrun backend run` itself, so the code is the standalone one. *Manual (operator):* Run → Start Backend / Stop Backend in the IDE window — not driven by the agent |
| AC10 frontend secret stops build, backend's does not | `backend_secret_not_scanned::a_frontend_secret_stops_the_build_and_the_backends_own_does_not` | pass; `connect-src 'self' wss://api.example.com` |

Routes (`loopback::routes_are_closed`, Leptos table): `GET /health` 200; `POST /health`, `/dl/<bad>`, `/dl/../etc/passwd`, `GET /ws` and
`GET /` without upgrade, `/admin`, `/api/anything`, `/ws/extra`, `/health/` → 404; upgrade `/` and `/ws` → 101.
Cross-spec risk R1 (the player's `{"type":…}` envelope, `cobolt-web-player` `net.rs`, vs `proto.rs` `{"t":…}` with `hello`) is
**recorded, not fixed**: WASM-014/WASM-016 own it.

Sweep (T14) with `--no-fail-fast`, every `test result` line summed (`df` before: 37 GiB free; after: 22 GiB):

| Crate | `test result` lines | Passed | Failed | Ignored |
|---|---|---|---|---|
| cobolt-runtime | 183 | 1554 | 0 | 29 |
| cobolt-wire | 4 | 23 | 0 | 0 |
| cobolt-server | 7 | 70 | 0 | 0 |
| cobolt-compiler | 11 | 207 | 4 | 5 |
| cobolt-cli | 21 | 61 | 1 | 5 |
| cobolt-project-tools | 12 | 158 | 0 | 1 |
| cobolt-web-bundle | 7 | 30 | 0 | 0 |
| cobolt-ide (`--bin cobolt-ide`) | 1 | 1877 | 5 | 4 |

The 10 failures, none in code this run touched:
- **cobolt-compiler (4):** the SDK crate set does not ship `cobolt-decimal`/`cobolt-numedit` — `sdk_crate_set_is_closed_under_path_dependencies`
  and three `resolve_main_tests`; the same four as the previous run.
- **cobolt-cli (1):** `aws_hosts::the_hosted_demos_pass_over_https_with_nothing_on_the_path` — the embedded `aws-agent-memory-demo`
  answered `PASS 005 FAIL 002` during the sweep; re-run alone it **passes** (131.7 s). Sensitive to machine load, not to this change.
- **cobolt-ide (5):** the same five as the previous run — `every_document_ships_in_every_language` (docs translations removed under
  the 2.x.x rule), `every_completion_description_exists_in_six_languages` (12 English-only descriptions, none touched here),
  `claude_validates_the_written_bundle` (external `claude` validator), `compatible_and_reserved_collisions_still_refuse` and
  `live_resolver_verdicts` (live crates.io).

`cargo fmt --check` is not clean for the workspace (887 files written in wide lines before this run); the new files follow the
surrounding wide style and nothing was reformatted. Credential scan of the diff and the three new files: nothing found (the test's
planted key is built from fragments). System KB: *Calling the backend* names Run → Start/Stop Backend and the live log;
`chunked.data` regenerated (3554 records, 12 738 560 → 12 742 656 bytes). `README.md` and `docs/` untouched.

- **T15 — D21: the asymmetric channel and the retry policy** (R24–R26; AC11–AC12) — the key pair generated at application start and exchanged in the handshake (with WASM-014 T19b), the ciphertext capture of AC11, the three timed attempts and the exception of AC12; a Finalize of its own closes it (version, CHANGELOG, the tests of every touched crate). Added 2026-10-24 by the operator's ruling, after T14 had finalized the spec.
  - **Split (operator, 2026-10-30)** into T15a and T15b: the retry policy does not depend on the encryption, and WASM-005 T18b waits only for it.
### Grounding read for T15a and T15b (2026-10-31, path and symbol)

`crates/cobolt-wire/Cargo.toml` (no `[features]`; deps `cobolt-ast`, `serde`, `serde_json` only), `src/contract.rs` (`PROTOCOL = 1` `:32`,
`PROTOCOL_MIN = 1` `:35`), `src/proto.rs` (`Hello` `:30`, `ServerHello` `:168`), `src/error.rs` (`NO_CONNECTION` 1000 `:94`, `CLOSED` 1001
`:95`, `TIMEOUT` 1002 `:96`, `BAD_MESSAGE` 4001 `:106`, `PROTOCOL_UNSUPPORTED` 6002 `:112`).
`crates/cobolt-web-player/src/net.rs`: `PROTOCOL = 1` `:64`; `trait Socket` `:80` (`send(&mut self, text: &str)` — text only); `Out` `:88`
(`Answer`, `State(LinkState)`, `Warning`); `LinkState` `:98` (`Closed`, `Connecting`, `Greeting`, `Authenticating`, `Ready`, `Backoff`);
`Config` `:111` (`call_timeout_ms` 30 000, `backoff_start_ms` 250, `backoff_max_ms` 10 000); `Client` `:193` (`attempt`, `retry_at`, `pub
jitter`); `request_as` `:367` (the `Closed if wanted` and `Backoff` arms); `on_message(&mut self, text: &str)` `:462`; `on_close` `:557`
(sent non-idempotent calls fail 1001 at once; the back-off "a failed attempt that leaves nothing waiting stops there" `:593-607`);
`tick` `:618` (call timeout from `sent_at`, then `retry_at`); `next_deadline` `:639`; tests `each_failure_is_mapped_onto_the_closed_list`
(asserts "nothing waits: no retry loop" `:874`), `a_safe_program_is_repeated_once_after_a_drop_and_an_unsafe_one_fails_with_may_have_run`,
`a_call_in_flight_when_the_connection_drops_fails_and_a_later_one_does_not`, `reconnection_backs_off_exponentially_and_resets_when_it_works`,
`jitter_stays_within_a_quarter_of_the_base`. `src/api.rs`: `impl Socket for WsSocket` `:251` (`send_with_str`), `set_onmessage` `:274`, the
`Out` dispatch `:441-461`, `seed_session` from `js_sys::Math::random` `:861`. `Cargo.toml`: `web-sys` features already include `Crypto`;
its own `Cargo.lock` (`getrandom` 0.4.3, `js-sys`/`web-sys` 0.3.106). `tests/dependency_guard.rs`
(`the_browser_build_links_no_c_toolchain_no_tls_stack_and_no_second_transport` `:108`, `the_licence_and_msrv_rules_read_spdx_and_versions_as_written`
`:213`), `tests/freshness.rs` (`REBUILD` = `cargo run --manifest-path crates/cobolt-web-player/Cargo.toml --example build_player -- --out
assets/web-player` `:25`).
`crates/cobolt-server/src/client.rs`: `ClientConfig` `:90` (`connect_timeout` 10 s, `call_timeout` 60 s, `reconnect_deadline` 20 s `:112`/`:128`);
`WsClient` `:193`, `WsClient::new` `:218`; the upload's own resume loop (`attempts < 6`, 200 ms steps, `:296-:373`); `install_default_from_env`
`:513`; `ensure` `:734` (four quick attempts, `:740`/`:757`); `on_disconnect` `:897` (100 ms doubling to 5 s within `reconnect_deadline`,
`:922-:953`). `src/serve.rs`: `Shared` `:90`, `session` `:638`, the `Message::Binary` refusal `:714` ("binary frames are not part of the
protocol"), `on_hello` `:956`. `tests/client_contract.rs` (`cfg.reconnect_deadline = 10 s` `:191`; `ac6_a_call_to_a_program_declared_safe_is_repeated_once_after_a_lost_connection`
`:431`), `tests/loopback.rs` (`concurrent_callers_…` `:420`, `the_log_has_neither_…` `:462`), `tests/support/mod.rs` (`Raw::hello` sends
`protocol: 1` `:337`), `tests/dependency_guards.rs` (5 tests, `:51`–`:106`).
`crates/cobolt-web-bundle/examples/mock_backend.rs` (its `hello` answers `protocol: 1` `:216`), `tests/e2e_chrome.rs`;
`crates/cobolt-compiler/tests/compiled_app_calls_backend.rs` (`a_built_application_calls_its_backend_as_rcrun_run_does` `:60` — the
compiled-binary parity path), `tests/backend_build_wasm003.rs` (its minimal client); `crates/cobolt-compiler/src/lib.rs` System KB section
*Calling the backend* `:5206` ("the reconnection and the token refresh are the runtime's" `:5226`); `specs/WASM-ABI-v1.md` (rows A10–A18;
A15 = the player speaks `proto.rs`); `crates/cobolt-ide/src/version.rs` (`VERSION = "2.0.102"` `:22` today).
`Cargo.lock` (main): `curve25519-dalek` 4.1.3, `hkdf` 0.12.4, `sha2` 0.10.9 and 0.11.0, `chacha20` 0.10.0, `aead` 0.6.1, `zeroize` 1.8.2,
`getrandom` 0.2.17/0.3.4/0.4.2; **no** `x25519-dalek`, **no** `chacha20poly1305`.
Cross-spec state (each spec's tasks.md, 2026-10-31): WASM-016 **T29b** (the modal, R20–R21) open; WASM-012 **T25b** (device modal) open;
WASM-014 **T19b** (handshake members, R18) open; WASM-013 **T24b** (R17 threat model) open; WASM-005 **T18b** waits for T15a.

### T15a — the retry policy and the blocked state (R25, R26, R7, R8; AC12; plan §2.8, D11–D13) — *first: depends on nothing in T15b*

Working assumptions the steps follow (plan §7, **to confirm**, not decided here): **O6** an attempt's "timeout" is its window, waited out when
the attempt fails early; **O7** a first connection that a call needs runs the cycle too; **O8** after the give-up the application's next call
starts a new cycle; **O9** the Rust client runs the same policy and exposes the state, but no desktop modal is drawn.

- [x] **T15a-1 — One retry policy in `cobolt-wire`** (R25; D11)
  - *Result (2026-10-31):* `cobolt_wire::retry::Policy { first_ms, step_ms, attempts }` (fields in milliseconds, so the player and the
    Rust client share one plain value), `Default` = 10 000 / 10 000 / 3, `window(n)` and `cycle_ms()`; no feature, no dependency.
    `the_windows_are_10_20_30_and_there_is_no_fourth` prints `windows 0..=4: [None, Some(10000), Some(20000), Some(30000), None];
    cycle 60000 ms`; `a_shorter_policy_keeps_the_same_shape` pass. cobolt-wire 31 + 1 + 1 + 0 pass, 0 fail.
  - Files: `crates/cobolt-wire/src/retry.rs` (new), `crates/cobolt-wire/src/lib.rs` (`pub mod retry;`).
  - Do: `pub struct Policy { first: Duration-as-ms, step, attempts }` with `Default` = 10 000 / 10 000 / 3 and `fn window(&self, n) ->
    Option<u64>` (attempt *n* ≥ 1 → `first + (n-1)·step`; `None` after `attempts`). Plain arithmetic: no feature, no dependency (the crate's
    "deliberately small" rule holds). Not a project setting (R25 fixes the numbers); tests construct a shorter one.
  - Verify: `cargo test -p cobolt-wire retry --no-fail-fast` — `retry::tests::the_windows_are_10_20_30_and_there_is_no_fourth` (prints the
    three windows); `cargo test -p cobolt-wire --no-fail-fast` green.

- [x] **T15a-2 — The player's connection runs the policy and reports *blocked* / *unreachable*** (R25, R26, R7; D12, D13; O6–O8)
  - *Result (2026-10-31):* `net.rs`: `LinkState::Retrying { attempt, window_ends }` replaces `Backoff`; `Config::retry` (=
    `cobolt_wire::retry::Policy::default()`) replaces `backoff_start_ms`/`backoff_max_ms`; `Client::jitter` gone. The shape chosen for
    T29b/T25b: two new `Out` variants, `Out::Blocked(bool)` and `Out::Unreachable(bool)` (`false` again once a later connection works),
    and `Client::blocked()` / `unreachable()`. Every attempt carries its window (`Attempt`), the first connection included: a break of a
    `Ready` connection blocks and opens attempt 1 at once; an attempt refused early waits its window out (`Retrying`), one still
    unfinished at its window's end is closed and counted failed (O6); a failed first connection with a call waiting joins the cycle as
    attempt 1, one nobody waits for stops there (O7, no knocking); a call while `Closed` after a give-up starts a new cycle (O8). Sent
    calls are marked `lost` at the break and decided in `ready()`; a call's timeout does not run while blocked and its `sent_at` is
    moved on by the blocked time at `unblock` (D12). Messages carry no time, so `ready()` uses the latest event time (`Client::clock`,
    the `on_open`'s). `api.rs`: `Inner::net_blocked`/`net_unreachable` keep the latest values, read by `Player::stats()` (`"blocked"`,
    `"unreachable"`) — no DOM, no caption. *Outside the task's files, needed to compile:* `dom/mod.rs` `net_state` matches
    `Retrying { .. }` where it matched `Backoff` (same "Reconnecting…" badge).
    Tests: `reconnection_backs_off_exponentially_and_resets_when_it_works` and `jitter_stays_within_a_quarter_of_the_base` replaced by
    `a_break_is_retried_at_10_20_30_seconds_then_given_up` (attempts opened at `[0, 10000, 30000]` ms after the break; transitions
    `blocked=true, blocked=false, unreachable=true`; two sent calls 1001 `may_have_run: true`, the one made while blocked 1000, no 1002
    during the 60 s cycle; no fourth `open` by 600 s; the next call opens attempt 1 of a new cycle and `unreachable=false` on success),
    `success_in_attempt_2_repeats_the_safe_call_and_fails_the_other` (FUND-TOTALS repeated under its key, FUND-SAVE 1001, FUND-LIST
    sent; the timeouts land at 40 500 / 41 500 ms — 1 s and 0 s counted before the block), `a_failed_first_connection_runs_the_cycle_only_when_a_call_waits`
    (`[10000, 30000]` after the first at 0). Updated, each to the new rule: `each_failure_is_mapped_onto_the_closed_list` (the `:874`
    assertion: "unreachable" is now 1000 at the give-up after three attempts, no fourth, and the next call opens a new cycle; "lost in
    flight" is 1001 when the next connection works), `a_call_in_flight_when_the_connection_drops_fails_and_a_later_one_does_not` (the
    lost call fails at the reconnection, not at the break), `a_safe_program_is_repeated_once_…` (both decided at the reconnection; the
    second drop fails at the next success), `a_call_that_is_never_answered_times_out_with_its_own_error` (a hung handshake is cut at
    10 000 ms, its call ends 1000 with the cycle). `--lib net` 21 pass; the player's suite: lib 305, `abi_host` 25, `dependency_guard`
    6, `exposed` 2, `grid_calls` 1, `sinks` 3 pass; `freshness` 4 pass / 1 fail (`the_shipped_player_was_built_from_the_source_that_is_here`,
    expected until T15a-4).
  - Files: `crates/cobolt-web-player/src/net.rs`; `crates/cobolt-web-player/src/api.rs` (the `Out` dispatch `:441-461` only).
  - Do: `LinkState::Backoff` → `Retrying { attempt, window_ends }`; `Config` loses `backoff_start_ms`/`backoff_max_ms` and gains `retry:
    cobolt_wire::retry::Policy` (defaults = R25); `Client::jitter` and `attempt`'s exponential use go (D13). `on_close`: any break of an
    established connection, or a failed connection a call needs (O7), starts a cycle; a failed attempt waits out its window (O6); sent calls
    are **not** failed at the break any more — they wait for the cycle's end (D12, plan §2.8). `tick`: a call's timeout is paused while
    `Retrying` (its elapsed time stops counting); the next attempt starts at `window_ends`. On success (`ready`): idempotent sent calls repeated
    once under their key; other sent calls fail *transport* `CLOSED` 1001 `may_have_run: true`; unsent calls go out. On the third failure:
    every waiting call fails 1001 `may_have_run: true` if sent, `NO_CONNECTION` 1000 if never sent, message "the backend could not be reached";
    no further attempt on its own; the next `request` starts a new cycle (O8). `Out` reports *blocked* on at the break and off at success or
    give-up, and *unreachable* at give-up (one new `Out` variant each, or `Out::State` carrying them — the shape is this task's; WASM-016 T29b
    and WASM-012 T25b read it). `api.rs` keeps the latest *blocked*/*unreachable* on its connection object for T29b; no DOM, no caption here.
  - Tests (replacing `reconnection_backs_off_exponentially_and_resets_when_it_works` and `jitter_stays_within_a_quarter_of_the_base`; the
    `:874` assertion of `each_failure_is_mapped_onto_the_closed_list` and `a_call_in_flight_when_the_connection_drops_fails_and_a_later_one_does_not`
    updated to the new rule, each change stated in the result): on the simulated clock — a break → *blocked* on, attempts at 10 000 / 20 000 /
    30 000 ms, an early refusal waits out its window, no fourth `open`; success in attempt 2 → *blocked* off, the idempotent call repeated once
    with its key, the other failed 1001 `may_have_run: true`, an unsent one sent; give-up → 1000/1001 on every waiting call, *unreachable* on;
    a 30 s call timeout does not fire during a 60 s cycle; a call after give-up starts a new cycle. Each test prints its transitions and times.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib net --no-fail-fast`, then the player's whole suite
    (`cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`; `freshness` is expected red until T15a-4).

- [x] **T15a-3 — The Rust client runs the same policy** (R25, R7; D11, D13; O9; interpreter/binary parity)
  - *Result (2026-10-31):* `client.rs`: `ClientConfig::retry` replaces `reconnect_deadline`; `Actor::cycle` is the policy (each `open` given
    its window by `tokio::time::timeout`, then `sleep_until` the window's end whatever the outcome; a refusal, a bad token or
    `NOT_CONFIGURED` ends it with its own error, as `ensure` did); `ensure` runs it for a first connection (O7, blocked from its first
    failure) and `on_disconnect` for a break (blocked at once). `on_disconnect` decides the calls at the end: success → a safe call
    repeated once, a sent unsafe one 1001 `may_have_run: true`, an unsent one sent; give-up → sent 1001, unsent 1000, "the backend could
    not be reached"; a call the backend had just refused by name (`conn_error`, kept from before) is still told at once. Deadlines move on
    by the blocked time (D12). *Found, not named by the plan:* commands that wait in the actor's channel during a cycle are failed 1000
    at the give-up (`refuse_queued`) — without it each would start its own 60 s cycle; and the upload/download resume loops stay
    (working assumption of the task): an `rpc` is still failed at the break and its loop's next try goes through the policy.
    `WsClient::link_status()` → `LinkStatus { blocked, unreachable }` (O9: no desktop modal). The `ensure` four quick attempts and the
    100 ms back-off are gone. *Test support:* `tests/support/mod.rs` `Proxy` records when each connection reached it (`accepted()`), the
    "listener on the port"; the backend is "stopped" by the proxy's existing `PROXY_REFUSE` (the TLS handshake fails at once) rather than
    by stopping and rebinding the server on one port. `client.rs`'s own `an_over_limit_call_fails_before_anything_is_sent` passes a
    100 ms policy (it reaches a closed port on purpose).
    Tests: `loopback::a_broken_connection_is_retried_by_the_policy_then_given_up` (policy 1.5/3/4.5 s; back before attempt 2: connections
    at `[1, 1505]` ms after the break, calls ended after 1544 ms, SAFE-READ repeated, UNSAFE-WRITE 1001 `may_have_run: true`, blocked seen
    during; stopped for good: connections at `[2, 1504, 4508]` ms, the sent call 1001 after 9417 ms, the call made while blocked 1000,
    given up at 9010 ms (cycle 9000), `unreachable: true`, no fourth connection 2 s later) and
    `loopback::the_retry_windows_are_10_20_30_seconds` (spec values: attempts at 0.00 / 10.01 / 30.01 s, windows 10.00 / 20.00 / 30.00 s,
    given up at 60.01 s, transport 1001 to the waiting call — its 60 s call timeout did not fire — 0 connections after). `client_contract`
    runs with `retry = 2/4/6 s` in place of `reconnect_deadline = 10 s`, `ac6_…_repeated_once_after_a_lost_connection` unchanged.
    cobolt-server: lib 14, `client_contract` 13, `config_rules` 11, `dependency_guards` 5, `generated_backend` 5, `generated_controls` 6,
    `generated_refusals` 1, `loopback` 17, `remote_bindings` 1, `token_rules` 15 — all pass. `compiled_app_calls_backend` (it is
    `#[ignore]`d, so run with `--ignored`): pass, build 20.0 s, `ping=pong, who=tester`, TRANSPORT 1003 without a backend. `backend_cli` 5 pass.
  - Files: `crates/cobolt-server/src/client.rs`, `crates/cobolt-server/tests/client_contract.rs` (`:191`), `crates/cobolt-server/tests/loopback.rs`.
  - Do: `ClientConfig` loses `reconnect_deadline` and gains `retry: Policy`; `on_disconnect`'s 100 ms back-off and `ensure`'s four quick
    attempts are replaced by the policy's cycle with the same success / give-up rules as T15a-2 (sent non-idempotent calls fail 1001
    `may_have_run: true`, unsent 1000); a call's `call_timeout` is paused while the client retries (D12); `WsClient` exposes *blocked* /
    *unreachable* to its host (an accessor or an observer — no desktop modal, O9). *Found, not named by the plan:* the upload's own resume loop
    (`:296-:373`, six tries of 200 ms steps) — working assumption: it stays, because each try goes through the connection, which now follows
    the policy; stated in the result.
  - Tests: `loopback::a_broken_connection_is_retried_by_the_policy_then_given_up` (short policy, real sockets: backend stopped and restarted
    inside attempt 2 → a safe call completes, another is *transport* 1001; stopped for good → *transport* after the third, a listener on the
    port sees exactly three connections) and `loopback::the_retry_windows_are_10_20_30_seconds` — **one** run at the spec values (~60 s), the
    three windows measured and printed, no fourth connection (plan §5 R12). `client_contract` re-run with `retry` in place of
    `reconnect_deadline`; `ac6_…_repeated_once_after_a_lost_connection` unchanged in what it asserts.
  - Verify: `cargo test -p cobolt-server --no-fail-fast` (every `test result` line read);
    `cargo test -p cobolt-compiler --test compiled_app_calls_backend --no-fail-fast` (the compiled binary uses this client: parity);
    `cargo test -p cobolt-cli --test backend_cli --no-fail-fast`.

- [x] **T15a-4 — The prebuilt player is rebuilt** (plan §3, §5 R14)
  - *Result (2026-10-31):* this worktree's `target-player` was already warm (2.3 GB), so nothing was cloned; `df` 22 GiB free before.
    `REBUILD` ran clean. `plain/player_bg.wasm` 3 577 964 → 3 580 105 bytes, `threaded/player_bg.wasm` 3 407 343 → 3 409 177; the parts
    (pdf 217 081, canvas 654 936, mermaid 2 219 072) unchanged; `player.js` glue renumbered (62 lines each way); `player-build.json`
    rewritten. `freshness` 5 pass (the file has 5 tests now, not 3). `cobolt-web-bundle`: see the T15a-7 sweep.
  - Files: `assets/web-player/` (written by the build, never by hand), the fingerprint record the build writes.
  - Do: `df -h .` first; clone a warm `target-player` (a cold build is ~4 GB); run `REBUILD` (`tests/freshness.rs:25`). Report `player.wasm`
    size before and after.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test freshness --no-fail-fast` (3 pass);
    `cargo test -p cobolt-web-bundle --no-fail-fast`.

- [ ] **T15a-5 — The blocked state reaches the page in the browser** (R26; AC12 browser half) — *runs with or after WASM-016 T29b*
  - *State (2026-10-31):* **open — WASM-016 T29b is not done** (`specs/WASM-016-…/tasks.md:423`, unchecked). What exists is in place
    for it: `Player::stats()` `blocked`/`unreachable`. The updated `e2e_chrome` *backend lost* scenario already saw `blocked: true` in
    Chrome. `mock_backend.rs` was not changed: the e2e test's own backend already stops and restarts on one port.
  - Files: `crates/cobolt-web-bundle/examples/mock_backend.rs` (a way for the test to stop and restart it), `crates/cobolt-web-bundle/tests/e2e_chrome.rs`.
  - Do: against `mock_backend` stopped and restarted, assert the page sees *blocked* on/off and, when it stays down, the *unreachable* event
    (WASM-016 R21). The modal's look is WASM-016 AC13–AC14, not this task's. **If T29b is not done when T15a runs**, this task stays open, and
    T15a-7 records AC12's browser half as waiting on it — the rest of T15a is not held.
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_chrome --no-fail-fast` (Chrome; prints the transitions it saw).

- [x] **T15a-6 — System KB: the retry behaviour** (System KB rule; plan §8)
  - *Result (2026-10-31):* a new bullet, *A lost connection*, under *Calling the backend* in `crates/cobolt-compiler/src/lib.rs`. It
    covers: three attempts of 10/20/30 s, an early failure waiting out its window, the application reported blocked, no call timeout
    running, new calls waiting, the success rule (idempotent ones repeated, others 1001 `may_have_run`), and the give-up rule (1001 if
    sent, 1000 if not, "the backend could not be reached", the next call starting again). It also says there is nothing to set.
    *Stated:* the KB does **not** promise the "connecting" modal, because WASM-016 T29b and WASM-012 T25b have not drawn it yet. Whoever
    draws it adds it to the KB. `build_chunked_kb`: 3664 records from 8 documents, `chunked.data` 13 357 056 → 13 234 176 bytes (the
    tool compacts the store). `prebuilt_chunked_kb_matches_the_published_documentation` pass.
  - Files: `crates/cobolt-compiler/src/lib.rs` (*Calling the backend*, `:5206`; the sentence at `:5226`), `assets/knowledge/chunked.data` (regenerated).
  - Do: what a developer sees — a broken connection is tried again three times (10, 20, 30 s), the application is blocked meanwhile (the
    "connecting" modal on the web and the device), then every waiting call reaches `ON EXCEPTION` with class *transport*; only programs declared
    `idempotent` are repeated. No new `COBOL::"…"` built-in. `cargo run -p cobolt-ide --example build_chunked_kb` in the same change.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`; record count and byte size printed.

- [x] **T15a-7 — Evidence and Finalize for T15a** (AC12; all of T15a) — see *Evidence (T15a)* below; T15a-5 stays open (WASM-016 T29b)
  - Files: this file (an *Evidence (T15a)* table: AC12 row, the test, the measured windows), `crates/cobolt-ide/src/version.rs` (bump `z`
    **once**: `2.0.102` today → the next free `2.0.z` at the time), `CHANGELOG.md` (one entry **above** the previous heading,
    `## [PowerRustCOBOL 2.0.z] — <date>`, *Written on features-2x*, listing T15a-1…6). **No documentation task:** `README.md` and `docs/`
    are not touched on 2.x.x. No new `Tr` key is expected (plan §8); if one was added, it is in all six tables.
  - Verify, `--no-fail-fast`, every `test result` line read and summed: `cargo test -p cobolt-wire`, `cargo test -p cobolt-server`,
    `cargo test -p cobolt-compiler`, `cargo test -p cobolt-cli --test backend_cli`, `cargo test -p cobolt-web-bundle`,
    `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml`, `cargo test -p cobolt-ide --bin cobolt-ide` (known reds on 2.x.x
    reported as pre-existing, by name); `df -h .` before; the credential scan of the diff. Manual (operator, never driven by the agent): a
    Web project's backend started from the IDE, the page open, the backend stopped and restarted — modal and calls as AC12 says.
  - Unblocks WASM-005 T18b.
  - [x] *Done 2026-10-31, `2.0.102` → **`2.0.112`*** (once). 2.0.103–2.0.111 were taken on sibling branches; the main checkout already
    carries 2.0.111. One CHANGELOG entry sits above 2.0.102. No `Tr` key added. `README.md` and `docs/` untouched.

### Evidence (T15a), 2026-10-31

macOS (Darwin 24.5, aarch64), debug builds, real sockets on loopback; the numbers are what the tests printed. **Windows and Linux were
not run.**

| AC | Proved by | Result |
|---|---|---|
| AC12 — three timed attempts, then the exception (Rust client, spec values) | `loopback::the_retry_windows_are_10_20_30_seconds` | pass; attempts at 0.00 / 10.01 / 30.01 s after the break, windows **10.00 / 20.00 / 30.00 s**, given up at 60.01 s, the waiting call `TRANSPORT` 1001 (its 60 s call timeout did not fire), no fourth connection |
| AC12 — back before the third: safe call completes, the other `transport` | `loopback::a_broken_connection_is_retried_by_the_policy_then_given_up` (1.5/3/4.5 s policy) | pass; reconnected at 1505 ms, SAFE-READ repeated, UNSAFE-WRITE 1001 `may_have_run: true`; stopped for good: connections at 2 / 1504 / 4508 ms, sent 1001, queued 1000, `unreachable` |
| AC12 — the player, simulated clock | `net::tests::a_break_is_retried_at_10_20_30_seconds_then_given_up`, `success_in_attempt_2_…`, `a_failed_first_connection_…` | pass; attempts opened at 0 / 10 000 / 30 000 ms, `blocked=true, blocked=false, unreachable=true`, no 1002 during the cycle |
| AC12 — the browser half | `e2e_chrome` *backend lost* scenario (updated, see below) | **partial, T15a-5 open.** In Chrome the player reported `blocked: true` while it retried, the lost call reached `ON EXCEPTION` with 1001 `may_have_run: true` when attempt 2 reconnected, and a later call answered `pong`. *Not shown:* the modal disappearing and the *unreachable* event (WASM-016 AC13/R21) — they wait for WASM-016 T29b |
| Parity (compiled binary) | `compiled_app_calls_backend` (`--ignored`) | pass; built in 20.0 s; it links this client |

**`e2e_chrome` changed with the rule.** Its *backend lost (AC5)* scenario waited 20 s for the in-flight call to fail **before**
restarting the backend. That is the behaviour T15a-2 retires, so the sweep's first run failed it (0/1, 63 s). The scenario now restarts
the backend after 1.2 s, then expects the `ON EXCEPTION` at the reconnection, plus `stats().blocked`. Result: 28 passed, 0 failed, in
both placements.

Sweep, `--no-fail-fast`, every `test result` line summed (`df` before: 22 GiB free; after: 12 GiB):

| Crate | Passed | Failed | Ignored |
|---|---|---|---|
| cobolt-wire | 33 | 0 | 0 |
| cobolt-server | 88 | 0 | 0 |
| cobolt-compiler | 228 (+ `compiled_app_calls_backend` 1 with `--ignored`) | 0 | 5 |
| cobolt-cli (`--test backend_cli`) | 5 | 0 | 0 |
| cobolt-web-bundle | 59 (+ `e2e_chrome` 1 on re-run) | 1 (the first run's `e2e_chrome`, above) | 0 |
| cobolt-web-player (`--manifest-path`) | 342 + `freshness` 5 | 0 | 0 |
| cobolt-ide (`--bin cobolt-ide`) | 1909 | 6 | 5 |

The six cobolt-ide failures are not this change:
- Five are the known 2.x.x reds: `claude_validates_the_written_bundle`, `every_document_ships_in_every_language`,
  `compatible_and_reserved_collisions_still_refuse`, `live_resolver_verdicts` and `every_completion_description_exists_in_six_languages`.
- The sixth, `panels::web_preview::tests::a_web_view_that_cannot_start_leaves_the_browser_not_nothing`, **passes when re-run alone**
  (a timing test under load; the file is untouched).

`prebuilt_chunked_kb_matches_the_published_documentation` passes. The credential scan of the diff and of the new `retry.rs` found
nothing. `cargo fmt --check` was not made clean: the workspace is written in wide lines, and nothing was reformatted.
**Manual (operator, never driven by the agent):** start a Web project's backend from the IDE, open the page, stop the backend and
restart it within a minute. The calls should behave as AC12 says. The modal is not there yet (WASM-016 T29b).

### T15b — the sealed channel (R24; AC11; plan §2.7, D7–D10) — *after T15a*

**Gate before T15b-1 starts:** WASM-013 **T24b** (R17's text) and WASM-014 **T19b** (the handshake members) done, and **O10** confirmed there:
the plan proposes the keys in the client's `hello` and the server's answer (D8), against WASM-014 R18's wording "the frontend's in its reply".
If WASM-014 decides the other order, T15b-3's handshake follows WASM-014, not D8. **O5** (scope: the WebSocket only; `/health`, `/dl/<token>`
and the identity routes stay TLS-only) is the working assumption, **to confirm**.

*Gate checked 2026-10-31 — not met; the /implement run of that day stopped here.* WASM-013 T24b (`specs/WASM-013-security-architecture/tasks.md:490`)
and WASM-014 T19b (`specs/WASM-014-backend-contract-and-evolution/tasks.md:464`) are both unchecked, and O10 is unconfirmed. T15b-1…6 not started.

- [ ] **T15b-1 — The four crypto crates, probed and guarded first** (R24; tech steering; plan §5 R8)
  - Files: root `Cargo.toml` (`[workspace.dependencies]`: `x25519-dalek` with `static_secrets` + `zeroize`, `hkdf`, `sha2`,
    `chacha20poly1305`), `crates/cobolt-wire/Cargo.toml` (feature `channel`, off by default, naming them and `zeroize`),
    `crates/cobolt-server/Cargo.toml` (`server` and `client` turn on `cobolt-wire/channel`; `loader` does not),
    `crates/cobolt-web-player/Cargo.toml` and its `Cargo.lock` (the **same** versions), `crates/cobolt-server/tests/dependency_guards.rs`,
    `crates/cobolt-web-player/tests/dependency_guard.rs`.
  - Do: probe in a throwaway Cargo project **outside** the repository first: one RustCrypto generation for the four (the one fitting `sha2`
    0.10 / `hkdf` 0.12 / `curve25519-dalek` 4 unless a newer one is required), each `rust-version` ≤ 1.92, no `ring`/`aws-lc*`/`openssl*`, builds
    for `wasm32-unknown-unknown`. Add the dependencies and nothing that uses them. Guards: the four present in the `server`, `client` and player
    closures; absent from `loader` and from the compiler's. **If a barred crate or a newer MSRV is unavoidable, stop and report; no guard is
    weakened.**
  - Verify: `cargo test -p cobolt-server --test dependency_guards --no-fail-fast` and
    `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test dependency_guard --no-fail-fast` (closure sizes, the four names,
    highest MSRV printed); `cargo check -p cobolt-wire` and `cargo check -p cobolt-server --no-default-features --features loader` clean;
    `Cargo.lock` diff: no existing crate changes version.

- [ ] **T15b-2 — `cobolt_wire::seal`** (R24; D7, D9)
  - Files: `crates/cobolt-wire/src/seal.rs` (new, `#[cfg(feature = "channel")]`), `crates/cobolt-wire/src/lib.rs`.
  - Do: a key pair from 32 caller-given bytes (the module takes no randomness itself, so it builds unchanged for `wasm32`); the agreement
    (an all-zero shared secret refused); the transcript (protocol, both public keys, both nonces, fixed order); HKDF-SHA256 with salt
    `SHA-256(transcript)` → two 32-byte keys, `info` `"prc-channel/1 c2s"` / `"prc-channel/1 s2c"`; `seal(&mut self, text) -> Vec<u8>` and
    `open(&mut self, bytes) -> Result<String, CallError>` with the implicit per-direction 64-bit counter nonce (4 zero bytes ‖ counter BE).
    Secrets zeroised on drop, no `Debug`, no `Serialize`.
  - Verify: `cargo test -p cobolt-wire --features channel seal --no-fail-fast` — the RFC 7748 X25519 vector and the RFC 8439
    ChaCha20-Poly1305 vector through the wrappers; both sides derive the same two keys; two connections of the same key pairs derive different
    keys; tampered, replayed, reordered, truncated frames refused; a low-order peer key refused; a key pair from other bytes differs.
    `cargo test -p cobolt-wire --no-fail-fast` (without the feature) unchanged.

- [ ] **T15b-3 — Every speaker of the protocol goes to protocol 2, sealed, in one change** (R24, R11, R5; D8, D10; plan §5 R11, R9)
  - Files: `crates/cobolt-wire/src/proto.rs` (`Hello`, `ServerHello` gain `key`, `nonce`, base64, required from protocol 2 — as WASM-014 T19b
    defines them), `crates/cobolt-wire/src/contract.rs` (`PROTOCOL`, `PROTOCOL_MIN` → 2); `crates/cobolt-server/src/serve.rs` (`Shared` holds
    the process's `StaticSecret` from `getrandom`, made at server start; `on_hello` agrees; `session` seals/opens every data message after the
    `hello`s, the `:714` binary refusal becomes the text-after-handshake refusal; socket caps raised by the 16-byte tag so `max_frame_bytes`
    stays the plaintext limit); `crates/cobolt-server/src/client.rs` (key pair made in `WsClient::new`, shared by its clones);
    `crates/cobolt-web-player/src/net.rs` (`PROTOCOL` → 2; `Socket` gains a binary send and `Client` a binary receive; a sealed connection
    cannot start without key bytes from its owner), `crates/cobolt-web-player/src/api.rs` (`WsSocket` sends bytes, `binaryType = "arraybuffer"`;
    the 32 key bytes from **`crypto.getRandomValues`**, never `Math.random`, made when the application opens); `crates/cobolt-web-bundle/examples/mock_backend.rs`;
    `crates/cobolt-server/tests/support/mod.rs` (`Raw::hello` and its frames), the minimal client of
    `crates/cobolt-compiler/tests/backend_build_wasm003.rs`; `specs/WASM-ABI-v1.md` (one row after A18 amending A15: sealed frames, the two
    members, the module still sees A12 frames only); `assets/web-player/` rebuilt (as T15a-4) with sizes before/after.
  - Do: no key in the build, the bundle, `interface.json` or a log (R24, plan §2.1, §2.4). Refusals: protocol 1 → `PROTOCOL_UNSUPPORTED` 6002;
    protocol 2 without a usable `key`/`nonce`, a failed exchange, a frame that does not open, a text message after the handshake → *transport*
    and close. No plaintext mode in any start (D9). `proto.rs` inside the frames is unchanged, so call order, tokens, limits and idempotency
    are untouched.
  - Verify: `cargo test -p cobolt-server --no-fail-fast` (all earlier suites unchanged in what they assert);
    `cargo test -p cobolt-compiler --test backend_build_wasm003 --test compiled_app_calls_backend --no-fail-fast` (binary parity; size and cold
    build time printed against 71 s / 67.2 MB, plan §5 R5); `cargo test -p cobolt-cli --test backend_cli --no-fail-fast`;
    `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` (incl. a test that the key source is
    `getRandomValues`, and `freshness`); `cargo test -p cobolt-web-bundle --test e2e_chrome --no-fail-fast` against the sealed mock.

- [ ] **T15b-4 — The AC11 capture and the refusals, proven** (R24; AC11, AC6, AC7, AC8)
  - Files: `crates/cobolt-server/tests/loopback.rs` (tests only; a TLS-terminating recording proxy in the test).
  - Do / Verify: `cargo test -p cobolt-server --test loopback --no-fail-fast` —
    `the_channel_is_ciphertext_at_a_terminating_proxy` (only the two `hello`s are text; every later message binary; no recorded bytes contain
    the program name, a payload value, the token or `"t":`; after a restart of backend and client, different public keys and the first
    capture's frames refused in a new connection; message counts and the two key fingerprints printed);
    `a_connection_without_the_exchange_is_refused` (protocol 1 → 6002; no `key` → *transport*; text after the handshake → *transport*, closed);
    `the_frame_limit_is_the_plaintext_limit` (exactly `max_frame_bytes` passes, one more → 7000); `concurrent_callers_…` (AC7) sealed, its
    400-call time printed against the unsealed run; `the_log_has_neither_…` (AC8) extended: no key, shared secret or nonce in the log.

- [ ] **T15b-5 — System KB: the encrypted channel** (System KB rule; plan §8)
  - Files: `crates/cobolt-compiler/src/lib.rs` (*Calling the backend*), `assets/knowledge/chunked.data` (regenerated).
  - Do: the channel is encrypted on top of TLS with keys made at each start; nothing to set or write; a frontend and a backend built on
    either side of this change refuse each other with a *version* error. `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`.

- [ ] **T15b-6 — Evidence and Finalize for T15b** (AC11; all of T15b)
  - Files: this file (an *Evidence (T15b)* table: AC11 row and the re-run AC1–AC10 rows on the sealed channel), `crates/cobolt-ide/src/version.rs`
    (bump `z` **once** → the next free `2.0.z`), `CHANGELOG.md` (one entry above the previous heading, dated, *Written on features-2x*, listing
    T15b-1…5, stating that a Web bundle and a backend built before and after it do not talk). No `README.md`/`docs/` change; no new `Tr` key
    expected (if one was added, six tables).
  - Verify, `--no-fail-fast`, every `test result` line read and summed: `cargo test -p cobolt-wire` and `--features channel`,
    `cargo test -p cobolt-server`, `cargo test -p cobolt-compiler`, `cargo test -p cobolt-cli --test backend_cli`,
    `cargo test -p cobolt-web-bundle`, `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml`, `cargo test -p cobolt-ide --bin cobolt-ide`;
    `df -h .` before; the credential scan of the diff. State that Windows and Linux were not run (plan §5 R7) if they were not.

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
| AC9 IDE start/stop, binary identical | D-e, T10 |
| AC10 frontend secret stops build, backend's does not | T12 |
| AC1–AC10 (re-run sealed) | T15b-3, T15b-4, T15b-6 |
| AC11 ciphertext on the wire, a new key pair per start | T15b-2 (vectors, per-connection keys), T15b-3 (keys at start), T15b-4 (`the_channel_is_ciphertext_at_a_terminating_proxy`) |
| AC12 three timed attempts, then the exception | T15a-1 (windows), T15a-2 (player, simulated clock), T15a-3 (real sockets, spec values measured), T15a-5 (browser, with WASM-016 T29b) |

## Open items carried (not decided by this list)

- **O1** — decided by the operator 2026-10-23: the front runs on Leptos for real (T10a, done), plain `cargo build`; whether `cargo-leptos` must ever run the backend build stays **to confirm** (plan §7).
- **O2** — the words `per-call` / `per-connection`: plan's spelling; the visible text is in Tr keys (T9).
- **T10's hosting model** — decided by the operator 2026-10-23: a child `rcrun backend run` (T10, done).
- **D4 reading of R5** — loopback `ws` in a release start behind a reverse proxy; for the next `/clarify`.
- **O3** — WASM-INDEX D4/D5 still say "Open"; another file, left to its owner.
- **Plan §5 R1** — the player/server envelope mismatch: **closed** (plan C13, WASM-014 T6, ABI A15).
- **O5** — R24's scope: the WebSocket channel only (working assumption, T15b). **To confirm.**
- **O6** — R25's "timeout" is the attempt's window, waited out on an early failure (working assumption, T15a). **To confirm.**
- **O7** — a first connection a call needs runs the cycle (working assumption, T15a). **To confirm.**
- **O8** — after the give-up, the next call starts a new cycle (working assumption, T15a). **To confirm.**
- **O9** — the Rust client runs the policy and exposes the state; no desktop modal (working assumption, T15a-3). **To confirm.**
- **O10** — handshake order (plan D8) against WASM-014 R18's wording; WASM-014 T19b decides it before T15b starts. **To confirm.**
- **Not this spec's code:** the IDE's Output panel does not show the `connect-src` (T12; needs a `Tr` label); the modals (WASM-016 T29b,
  WASM-012 T25b); R17's threat-model text (WASM-013 T24b); no real identity provider contacted.
- **T5, the `hello` member** — the plan wants the client told by `hello` that a reconnection starts with fresh state; WASM-014's
  `hello.session` means the opposite direction and purpose (a client id that continues an idempotency session). Which member, if any,
  says "state is new" is WASM-014's to define; nothing was added to `proto.rs`.

## Done criteria
All acceptance criteria in spec.md are checked with the evidence of T13 (AC1–AC10), *Evidence (T15a)* (AC12) and *Evidence (T15b)*
(AC11, and AC1–AC10 re-run sealed); the tests of every touched crate pass, the System KB and
`chunked.data` agree, the six i18n tables have the new keys, and the version and CHANGELOG are updated once. Commits and pushes are the
pipeline's, not this list's.
