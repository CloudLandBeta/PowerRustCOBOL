<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The Backend project and the secure frontend–backend channel

- **Spec:** [spec.md](spec.md). **Status:** revised 2026-10-22 against the spec as it stands after the operator's answers of that day (commit 79400bd9). The 2026-10-20 work (M3) — the backend crate, the headless build, the channel and the token checks, tested over a real TLS loopback — stands; this revision adds what the new answers require (§0) and is **not implemented**. The contract half (WASM-014) is M4 and the identity half (WASM-009 backend) is M5 — see their plans.
- **Written:** 2026-10-20 by the implementing agent while the operator was away; **revised 2026-10-22** (this /plan run). Decisions the operator has since made are marked *operator*; the rest are this plan's, each with what it chose, why and what the alternative cost — for the operator to overturn.
- **Evidence** (§8) is the 2026-10-20 run; nothing is claimed there that was not run. The work of §0 has no evidence yet.

## 0. What changed on 2026-10-22, and why

| # | Spec change | Was (2026-10-20 plan / code) | Now (this plan) |
|---|---|---|---|
| C1 | **R2:** the server is **built on Leptos** (`cargo-leptos`, WASM-002 Q2) and carries the WebSocket channel **and the HTTPS endpoints**; the Leptos plumbing is invisible, the developer sees only COBOL | A hand-rolled HTTP front on `tokio` + `httparse` (`crates/cobolt-server/src/http.rs` `read_head`, `respond`; routing in `serve.rs` `serve_stream`, `health_loop`) | The HTTP front (accept → TLS → route → upgrade) moves onto Leptos's server integration (`leptos_axum` over `axum`/`hyper`); **everything after the upgrade is unchanged** (§2.2, §4 D1). How `cargo-leptos` enters the build is an open conflict with WASM-017 R17 (§7 O1) |
| C2 | **Q2 decided** (*operator*): statefulness is **a project property**, default fresh `WORKING-STORAGE` per call | Fresh per call, hard-wired (`crates/cobolt-server/src/exec.rs` `execute` builds a new `Interpreter` every call); the System KB says so unconditionally (`crates/cobolt-compiler/src/lib.rs`, section *Calling the backend*) | `[backend] state = "per-call" \| "per-connection"` in `cobolt.toml`, default `per-call`; `per-connection` keeps one run unit per connection (§2.6) |
| C3 | **Q3 decided** (*operator*): the backend **also serves ordinary HTTPS endpoints** | "One endpoint", `GET /health` (+ `/dl/<token>` from WASM-014 R15); a general router was called speculative | A real router (the Leptos/axum one of C1) with a fixed, closed route table; the identity endpoints of WASM-009 mount on it (§2.2) |
| C4 | Q4, Q5, Q6, Q7 **decided** (*operator*) as the plan had already assumed | Assumptions | Now binding; no design change. Q7 is read as *the binary refuses a non-loopback `ws` in every mode*, which removes `unsafe_plain_ws` (§4 D4) |
| C5 | — (found while grounding this revision) | Two lists of exported programs: the IDE writes `[backend] exports` in `cobolt.toml` (`crates/cobolt-compiler/src/project_kind.rs` `BackendSettings::exports`; edited in `crates/cobolt-ide/src/panels/web_settings.rs`), the server reads only `backend/exports.toml` (`crates/cobolt-server/src/backend.rs` `Backend::load`); the scaffold (`crates/cobolt-project-tools/src/create.rs`) writes no `exports.toml`, so a new project's backend does not start | `backend/exports.toml` is the only list (§3, §4 D5) |
| C6 | — (found) | The player speaks ABI amendment A8 (`{"type":"call",…}`, no `hello`; `crates/cobolt-web-player/src/net.rs`); the server speaks `{"t":…}` with a mandatory `hello` (`crates/cobolt-wire/src/proto.rs` `parse_client`). They cannot talk | Recorded as a cross-spec risk owned by WASM-014 (the wire) / WASM-016 (the player); not changed here (§5 R1) |

Kept unchanged: the security order of a connection and a call, tokens (§2.3), hardening (§2.4), cross-connection locking (§2.5), the System KB built-in, the CLI.

## 1. Decisions on the open questions

All seven questions are now decided by the operator; the spec has nothing left open. What the plan does with each:

| Q | Operator's decision | Design |
|---|---|---|
| **Q1** | `backend/` inside the Web or Mobile project | §2.1. One list of exports, in that folder (C5) |
| **Q2** | A project property, default fresh `WORKING-STORAGE` per call | `per-call` as built; `per-connection` added (§2.6) |
| **Q3** | Yes, ordinary HTTPS endpoints | A closed route table on the Leptos router (§2.2) |
| **Q4** | No program is safe to repeat by default | `idempotent = false` unless an export says `true` (`ExportDecl::idempotent`, `backend.rs`); semantics in WASM-014 |
| **Q5** | Development token in a development run only, never in a release build | As built: HS256 with a secret made fresh at every start, setting named `unsafe_dev_token_secret`; a release start refuses every `unsafe_*` (`crates/cobolt-server/src/config.rs`); the built binary always starts as release. Test `a_release_start_refuses_every_unsafe_setting_and_a_development_start_only_warns` |
| **Q6** | One binary per host, spec 041 | As built: `crates/cobolt-compiler/src/backend_build.rs` builds for `host_triple` only |
| **Q7** | Both: TLS in the binary, and behind a reverse proxy | As built (`native-tls` acceptor; loopback `ws` behind a proxy), minus `unsafe_plain_ws` (§4 D4) |

Further decisions of 2026-10-20 that still hold (unchanged): **TLS is `native-tls`** (`ring`, `aws-lc-rs` barred by `dependency_guards_063`); **JWT is `jsonwebtoken` 11 with `rust_crypto`**, used only to verify a signature and decode, every policy check ours; the macOS PEM/EC limitation (`Identity::from_pkcs8` takes RSA only, use `tls.pkcs12` for EC); **a closed set of failure classes with stable codes** (`crates/cobolt-wire/src/error.rs`); **an unknown program answers "no token" to an unauthenticated caller** so names cannot be enumerated; **strict configuration** (`deny_unknown_fields`, all faults reported together); **secrets print as nothing** (`Secret`'s `Debug`).

## 2. Design

### 2.1 The project and the build (R1–R4, R20, R22)
- **Project shape** (`crates/cobolt-server/src/backend.rs`): `backend/exports.toml`, `backend/src/*.cbl`, `backend/COPYBOOKS/`, `backend/backend.toml` (the deployment), `backend/contract/interface.json` (written by Build), `backend/contract/released/<version>.json`.
- **Exports** (R1): `exports.toml` is the whole of what the channel can run — `program`, `roles`, `public`, `idempotent`, `independent`, `long_running`, `stream`, `directions`, `deprecated` (`ExportDecl`). A program not in it is unreachable by any spelling. **New (C5):** the IDE's Backend settings section edits this file (§3), and the scaffold writes it.
- **Loading** parses and analyses every program, checks each export names a program, turns `USING`/`RETURNING` groups into wire layouts (`cobolt_wire::layout::from_decl`), builds the interface description; the exported program becomes the top-level program with the other backend programs nested in it.
- **Build** (`backend_build.rs`): read and check → write `interface.json` (R4) → compare with the last released contract (WASM-014 R5) → generate a Cargo project (`cobolt-server[server]`, `cobolt-runtime` with only the features the programs reach, the bundle by `include_bytes!`, `main` = `cobolt_server::standalone::main`) → build → install `bin/<project>-backend`. No `eframe`, no `egui`, no form host. **New (C1):** the generated project is unchanged in shape — the Leptos server lives inside `cobolt-server` behind its `server` feature, so the developer's generated Cargo project names no Leptos crate and contains no Leptos code (R2 "invisible"). **New (C2):** the bundle carries the `state` property.
- **The compiler links only the loader** (`cobolt-server` feature `loader`), so an application build links no socket, TLS, token or Leptos code; the guard checks it.
- **CLI** (`crates/cobolt-cli/src/main.rs` `cmd_backend`): `rcrun backend run|describe|check|release|build [dir]`. `run` additionally reads `[backend] state` from the project's `cobolt.toml` (the CLI already resolves `project_dir`).
- **Scaffold (R22):** `crates/cobolt-project-tools/src/create.rs` already writes `backend/src/health.cbl` (`BACKEND_STARTER`, `HEALTH`); it must also write `backend/exports.toml` exporting `HEALTH` as `public` and a development `backend/backend.toml` (loopback, no secrets). The authentication programs are WASM-009 M5 and are added to the scaffold when they exist.

### 2.2 The channel and the HTTPS surface (R2, R5–R8, R14, R16, Q3)
**Connection order (unchanged as a security model, re-hosted):** accept → connection limit → TLS (`native-tls`, `tokio-native-tls`) → HTTP request parsed by `hyper` within the handshake timeout and the header-size cap → **route** → for the upgrade: **Origin check** → upgrade with frame and message size capped → `hello` within the handshake timeout.
**Call order (unchanged):** connection rate → (non-public) valid token → program lookup → user rate → role → concurrency permits → ordering ticket → `spawn_blocking` run under a deadline → projection to the caller's contract.

- **Where Leptos enters (C1).** `cobolt-server` (feature `server`) builds an `axum::Router` through `leptos_axum`, compiled for the server side only (`ssr`), no client crate, no hydration, no pages. It is driven by our own accept loop: the `native-tls` stream goes to `hyper_util`'s connection server with upgrades enabled, so the connection limit, the TLS handshake timeout and the loopback/TLS rules stay ours. The upgrade route performs the Origin check, then takes the raw upgraded connection and hands it to `tokio_tungstenite::WebSocketStream::from_raw_socket` with today's `WebSocketConfig` caps, then calls the existing `serve.rs` `session` unchanged (it is already generic over `AsyncRead + AsyncWrite`). Everything from `on_hello` / `on_text` / `on_call` on is untouched.
- **Route table (Q3), closed:** `GET /health`; `GET /dl/<token>` (WASM-014 R15, `serve_download`); `GET /` and `GET /ws` with an upgrade; the identity routes WASM-009 defines (refresh cookie, provider redirect), mounted by that module on the same router; everything else **404**. Every HTTP answer carries the security headers already sent by `http.rs` `respond`. A developer cannot add routes in v1 (a public REST API is a non-goal); the endpoints are the platform's, not COBOL's.
- **What retires:** `http.rs` `read_head`/`respond` and the routing branches of `serve_stream`; `health_loop` becomes a second listener on the same router (optional plain loopback `health_listen`, kept).
- **Wire, keep-alive, Origin, limits:** unchanged — JSON text frames tagged `t` (`proto.rs`), pings every 20 s and a 60 s idle cut, exact Origin match (`*` refused at configuration), limits 1 MiB / 30 s / 20 per s burst 40 / 50 per s burst 100 / 8 / 64 / 1000, codes 7000–7004.

### 2.3 Tokens (R9–R13) — unchanged
`jwt.rs`: shape and 16 KiB cap → allowed algorithm (`RS256`/`ES256` default; never `none`, never HMAC except the development token) → key by `kid`, key `alg` must equal the token's → issuer, `exp`, `nbf`, `leeway_seconds` ≤ 300 → audience/app client → access token, never one carrying `at_hash`/`nonce`. Profiles `generic`, `cognito`, `keycloak`, `entra`. **R11:** the token travels as a first message `{"t":"auth","token":…}` — never a URL, query, header or sub-protocol; a call may carry a fresher token (R13, `authentication 2002` on expiry). Expiry re-checked at every call. JWKS (`jwks.rs`) TTL 3600 s, refresh never more often than 30 s whatever the cause, last keys kept up to a day during an outage. Claims through `COBOL::"WS-CLAIM"` (R12), read-only.
**New with C2:** in `per-connection` mode the claims are re-installed at **every** call (`Interpreter::set_remote_claims`, `crates/cobolt-runtime/src/interpreter/remote.rs`), so a token refreshed mid-connection is what the program sees; a call whose token names a **different subject** than the one that opened the session is refused (`authorisation`) rather than handed another user's `WORKING-STORAGE`.

### 2.4 Hardening (R15, R17, R18, R21) — unchanged
Strict payload reader (`Interpreter::group_from_json`) before the program starts; panic and run-time errors answered `server 9001` without detail; `LogRecord` has no field for a token, a password, a payload or a claim; secrets from `env:`/`file:`/`backend.toml`, never a form; `/health` body as today. **New:** the log record of a call gains the session mode (`per-call`/`per-connection`), not its contents.

### 2.5 Concurrency across connections (R19) — unchanged
`Interpreter::set_file_lock_wait` (`crates/cobolt-runtime/src/interpreter.rs`) takes a `<path>.lck` advisory lock per `OPEN` (exclusive except `INPUT`), waits up to the configured time, else status 93; held until `CLOSE` has flushed (the 2026-10-20 runtime fix). SQL relies on the database's isolation. **New with C2:** in `per-connection` mode a program may keep a file open between calls; the lock is then held across calls of that connection, and other connections wait or get 93 — correct and visible, and stated in the KB text. A dropped connection ends its run unit, which closes its files and releases its locks.

### 2.6 Statefulness as a project property (R18, Q2) — new
- **Property:** `[backend] state = "per-call"` (default, also when absent — existing projects unchanged) or `"per-connection"`, in `cobolt.toml`; shown in the IDE's Backend settings section; copied into the bundle by the build and read by `rcrun backend run`.
- **`per-call`:** today's `exec::execute`.
- **`per-connection`:** one session per connection holding one `Interpreter` per exported program it has called (each exported program is its own top-level program, §2.1). A call re-enters that interpreter: arguments read into `LINKAGE`, the program runs, `WORKING-STORAGE` persists as for a program `CALL`ed again in one COBOL run unit; `EXTERNAL` items are shared across the session's interpreters through one `ExternalStore` (`Interpreter::external_store`, and the constructor `Interpreter::with_external_store(program, store)`). Calls of one session are **serialised** (the concurrency permit per connection becomes 1 for stateful programs; `independent` is ignored and the build warns). The session ends with the connection; a reconnection starts a fresh session, and the client is told so by the `hello` answer (a `session: "new"` member, owned by WASM-014's protocol) — no state is ever resumed across connections, so R7's "never run twice" and Q4 are untouched.
- **Prerequisite to verify first (§5 R3):** that `Interpreter::run` (interpreter.rs) can be entered more than once on one instance with `WORKING-STORAGE` kept and SQL work ended per call (`sql_end` runs at the end of each `run`), and that the interpreter can move between `spawn_blocking` threads (`Send`). If either fails, the runtime change is a separate, stated task, not a workaround in the server.
- **Isolation (R18) in this mode** is per connection, never per program across users: two connections never share an interpreter.

## 3. Data / model changes
- **`cobolt.toml` `[backend]`** (`BackendSettings`, `crates/cobolt-compiler/src/project_kind.rs`): add `state` (string, default `per-call`). `exports` (a list of names) is **retired as a source** but still deserialised: when a project is opened whose `backend/exports.toml` is absent and whose `[backend] exports` is non-empty, the IDE writes `exports.toml` from it (each name, no roles, not public) and reports it in the Output panel; the old list is left in place, not deleted (user code is sacred). `ManifestView::backend_exports` (`crates/cobolt-compiler/src/lib.rs`) and the Web settings field are re-pointed at `exports.toml`.
- **`backend/exports.toml`:** unchanged schema; now also written by the IDE.
- **Bundle** (`crates/cobolt-server/src/bundle.rs`): gains `state`; the bundle is regenerated at every build, so no compatibility shim.
- **`backend.toml`:** `unsafe_plain_ws` removed (§4 D4). A file that still has it fails to start with a message naming the reason (`deny_unknown_fields` already makes it a start error; the message is made specific).
- **Wire:** the `hello` answer gains `session` (with WASM-014). No other format changes.

## 4. Key decisions & alternatives
- **D1 — Re-host the HTTP front on Leptos's server integration; keep the session code.** Why: R2 requires the server to be built on Leptos, and the security-relevant code (`session`, `on_call`, `jwt.rs`) has tests that must keep passing unchanged; only the part Leptos replaces (parsing, routing, upgrade) moves. Rejected: rewriting the session on `axum::extract::ws` (a second WebSocket type, the frame caps re-proved, every loopback test re-earned for no behavioural gain); keeping the hand-rolled front (contradicts R2).
- **D2 — Leptos inside `cobolt-server`, not in the generated project.** Why: R2's "invisible plumbing"; the generated project stays a thin `main`; one place to guard dependencies. Rejected: generating Leptos code per project (the developer's Build would then own Leptos source, and D10's "no per-project Leptos code" spirit is lost).
- **D3 — `per-connection` as one run unit per connection, serialised.** Why: it is COBOL's own `CALL`-again semantics, so nothing new to teach; serialisation removes intra-session races. Rejected: one shared instance per user across connections (survives reconnection but leaks across tabs and needs eviction); per-program-per-user caches (unbounded memory).
- **D4 — Remove `unsafe_plain_ws`.** Why: R5 accepts plain `ws` only on loopback; Q7's decision keeps "the binary refuses a non-loopback `ws` either way"; a development relaxation that serves `ws` on a LAN address contradicts both. A developer testing from a phone uses TLS with a development certificate. Rejected: keep it as a development-only warning (it is how an unencrypted backend reaches a network by mistake). *Reading recorded:* R5's "only … in a development run" and Q7's reverse-proxy mode together allow loopback `ws` in a release start **only** as the reverse-proxy arrangement; the spec's R5 wording should say so at the next `/clarify`.
- **D5 — `backend/exports.toml` is the single list.** Why: it carries roles, `public`, `idempotent`, directions — the `cobolt.toml` list of names cannot express R9/R12/Q4; the server and the compiler already read it. Rejected: making `cobolt.toml` the source (would need the full schema in the project manifest and a second parser).
- **D6 — The route table is closed.** Why: Q3 asks for endpoints, the non-goals exclude a public REST API; every endpoint is platform code reviewed with the security model.

## 5. Risks & mitigations
- **R1 — The player and the server do not speak the same envelope (C6).** The player follows ABI A8 (`type`, no `hello`); the server follows `proto.rs` (`t`, mandatory `hello`). Until aligned, end-to-end fact (2) of WASM-PLAN fails although AC1–AC8 pass with the Rust client. → A8 itself says the player's `net.rs` is a proposal until WASM-014 replaces it; alignment is WASM-014/WASM-016 work, flagged to their plans; this plan changes neither side.
- **R2 — Leptos's server closure may bring a barred crate** (`ring`, `aws-lc-*`, `rustls*`, `openssl*`) or raise the MSRV above 1.92. → `default-features = false` on every Leptos crate, `ssr` only; extend `crates/cobolt-server/tests/dependency_guards.rs` to the new closure and to "no Leptos in the loader's closure"; check `rust-version` of each added crate. If a barred crate is unavoidable, stop and report; do not weaken the guard.
- **R3 — Re-entering an interpreter (`per-connection`) may not keep `WORKING-STORAGE`, or `Interpreter` may not be `Send`.** → A first task proves both with a unit test; failure turns into a runtime task (with parity across the hosts) before the server work.
- **R4 — `per-connection` holds file locks across calls.** → Documented in the KB; per-call deadline still applies; a dropped connection releases everything; a test shows another connection getting status 93 rather than hanging.
- **R5 — The 2026-10-20 evidence is for the hand-rolled front.** → Every loopback test runs again on the Leptos front unchanged; the binary size and cold build time are measured again and reported against 53.1 MB / 29 s.
- **R6 — Two export lists in old projects (C5).** → The one-way, non-destructive migration of §3, with a test.
- **R7 — Windows and Linux were never run** (still true). → Code stays on `std`, `tokio`, `native-tls`, `hyper`; the gap is stated in the evidence, not claimed.

## 6. Test strategy
Kept, and run again on the Leptos front: `cobolt-server` `loopback` (11), `token_rules` (15), `config_rules` (10), `dependency_guards` (4), `client_contract`; `cobolt-cli` `backend_cli` (3); `cobolt-compiler` `backend_build_wasm003` (3). Each reports what it did today (counts, timings, sizes).
New:
- `loopback::routes_are_closed` — `/health` 200, `/dl/<bad>` refused, `/ws` and `/` upgrade, any other path 404, a non-upgrade `GET /ws` 404; reports each status. (C3)
- `loopback::a_large_or_slow_request_head_is_refused` — header over the cap → 431, a head that never ends → closed within the handshake timeout. (the `http.rs` behaviour, now on `hyper`)
- `config_rules::plain_ws_is_never_served_on_a_non_loopback_address` — replaces the `unsafe_plain_ws` case; also a `backend.toml` still naming it fails with a specific message. (D4)
- `dependency_guards`: no barred crate in the Leptos closure; no `leptos*`/`axum`/`hyper` in the loader's closure. (R2)
- Runtime: `interpreter_reentry_keeps_working_storage` (cobolt-runtime) — run one program twice on one instance, a counter answers 1 then 2; SQL work ends at each run. (R3)
- `loopback::per_connection_state_is_kept_and_never_shared` — two connections, interleaved calls of a counter: each sees 1, 2, 3 on its own; a reconnect starts at 1; a call with a token of another subject is refused; `per-call` still answers 1 every time. Reports the sequences. (R18, Q2)
- `loopback::a_file_held_open_across_calls_makes_others_wait_or_93`. (§2.5)
- `cobolt-project-tools` create test: a new Web project has `backend/exports.toml` exporting `HEALTH` public, and `rcrun backend check` on it passes. (R22, C5)
- `cobolt-ide`: opening a project with `[backend] exports` and no `exports.toml` writes the file and leaves `cobolt.toml` untouched. (C5)
- System KB freshness test after the KB edit (below).
Manual: `rcrun backend run` on a new Web project, `curl -k https://127.0.0.1:<port>/health`; IDE Start/Stop when AC9's IDE half lands.

## 7. Open items (none of the spec's questions is open)
- **O1 — How `cargo-leptos` enters (conflict between WASM-003 R2 and WASM-017 R17).** R2 names `cargo-leptos`; WASM-017 R17 says `cargo-leptos` exists **only in the product's build**. The backend is built on the developer's machine. **Decided by the operator (2026-10-23):** the front must use Leptos for real (routes through `leptos_axum` and server functions, tasks.md T10a), not hyper + axum with Leptos linked and unused; still built with plain `cargo build`, so `cargo-leptos` is not needed on the developer's machine and R17 holds; WASM-002's plan (§4.4) already treats `cargo-leptos` as the product's tool. **To confirm by the operator**; if `cargo-leptos` must run the backend build, the Backend Build gains a tool check (IDE strings ×6) and R17 needs an amendment.
- **O2 — `per-connection` names.** `state = "per-call" | "per-connection"` is this plan's spelling; the operator may prefer other words in the settings panel (the Tr keys carry the visible text).
- **O3 — WASM-INDEX D4 and D5 still say "Open"** although Q2 and Q3 are decided; the index is another file, left for its owner.
- **Not done (unchanged from 2026-10-20, not this spec's code):** the IDE's Start/Stop and Output panel (AC9, IDE half; `cobolt_server::dev::DevServer` is ready); R23/AC10 (`connect-src` is written by `crates/cobolt-web-bundle/src/csp.rs`; the frontend secret scan is WASM-002/010); a real identity provider was never contacted.

## 8. Steering compliance
- [ ] **i18n ×6:** new IDE strings — the `state` property label and its two values, the exports-migration notice, (O1) a missing-tool message — as `Tr` keys in all six languages.
- [x] **Generated-code contract:** `interface.json` and the generated backend Cargo project are regenerated at every Build, never hand-edited; nothing new is hand-maintained in them.
- [x] **Docs:** `README.md` and `docs/` untouched on 2.x.x; 2.x.x speaks in `specs/`.
- [ ] **System KB:** the *Calling the backend* section (`crates/cobolt-compiler/src/lib.rs`) says "every call runs in a fresh interpreter" — rewrite for the property, the `per-connection` lock behaviour and the single `exports.toml`; regenerate `assets/knowledge/chunked.data` in the same change. `COBOL::"WS-CLAIM"` already in `BUILTINS` (`crates/cobolt-runtime/src/builtins.rs`).
- [ ] **Interpreter/binary parity:** the backend is a host of the interpreter; any runtime change from R3 lands in `rcrun run-form`, embedded child forms, the compiled binary and the backend.
- [x] **PRIME DIRECTIVE:** Rust only; `cargo-leptos` (if O1 keeps it) is a tool, not repository source.
- [x] **User code is sacred:** the `[backend] exports` migration writes a new file and deletes nothing.
- [x] **Tech steering:** pure-Rust dependencies, OS TLS, no `ring`/`aws-lc-rs`, guarded by tests (spec 063 AC8 / 103).
- [ ] **Fix vs feature:** feature (2.x.x, bump z only); R19's locking stays a fix with its forward-port rule.

## 9. Evidence (2026-10-20, hand-rolled front; to be re-run on the Leptos front)

Everything below ran on macOS (Darwin 24.5, aarch64), debug builds, over real TLS sockets on loopback; the numbers are what the tests printed.

| Criterion | Where it is proved | Result |
|---|---|---|
| **AC1** one exported and one non-exported program build to one executable; only the first answers | `loopback::an_exported_program_answers_and_nothing_else_does` (nine spellings incl. `../` and a NUL: all `not-found 5000`); `compiler::backend_build_wasm003::the_built_backend_…` | pass. Built in **29 s** (debug, cold staging), **53.1 MB**; `--describe` prints `interface.json`; `--check` exits 2 on `unsafe_allow_any_origin` or a missing file; `GET /health` → 200 |
| **AC2** bad tokens refused, good accepted | `loopback::tokens_are_accepted_…`, `token_rules` (15) | pass: **14** bad tokens refused twice each; 1 good accepted; JWKS fetched **1** time, still 1 after 200 unknown `kid`s |
| **AC3** role refused before the program starts | `loopback::a_role_is_checked_before_the_program_starts` | pass |
| **AC4** foreign Origin refused; non-loopback `ws` refused | `loopback::a_foreign_origin_…` (5 refused 403); `config_rules::plain_ws_is_for_loopback_only_and_tls_is_all_or_nothing` | pass (the `unsafe_plain_ws` escape it tolerated is removed by D4) |
| **AC5** payload validation | `loopback::a_payload_that_does_not_fit_…` | pass |
| **AC6** distinct limit errors | `loopback::size_time_and_rate_limits_…` | pass: timeout at 703.5 ms (limit 700); burst 10 → 4 answered, 6 refused 7001; 8 kB frame over 4 kB → 7000 |
| **AC7** concurrent callers isolated | `loopback::concurrent_callers_…` | pass: 400 calls in 0.18 s, each its caller's own, every counter 1 (`per-call`) |
| **AC8** no token or password in the log | `loopback::the_log_has_neither_the_token_nor_a_field_…` | pass |
| **AC9** IDE start/stop; binary identical | `cli::backend_cli` (3) | **partial**: IDE half not done |
| **AC10** frontend secret scan | — | **not done** (WASM-002/010) |
| R19 file sharing | `loopback::calls_that_share_a_file_take_turns_and_none_is_lost` | pass: 24 writers, 473 ms, all status 00 (3 of 24 got 90 before the runtime fix) |

Test counts then: `cobolt-server` 41, `cobolt-cli backend_cli` 3, `cobolt-compiler backend_build_wasm003` 3. `loopback` once failed under machine load (`a_foreign_origin…`, 36 s) and passed 12 times alone — a timing sensitivity of the test client's 10-second waits. Dependencies then added: `tokio-tungstenite` 0.28 (`handshake`), `futures-util`, `native-tls`, `tokio-native-tls`, `jsonwebtoken` 11 (`rust_crypto`), `httparse`, `ureq` 2, `getrandom` 0.4, `base64` 0.22; test-only `rcgen` 0.14, `p256`, `rsa`, `signature`, `rand_core`. This revision adds the Leptos server crates (`leptos`, `leptos_axum`, `axum`, `hyper`, `hyper-util`) and, once the front moves, drops `httparse`.
